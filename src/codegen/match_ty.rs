//! The recursive shape-matching kernel: match_ty compares a shape template
//! (syn::Type) against a leaf type position-by-position, binding differing
//! idents as slots and resolving variadic segments. Split out of shape.rs
//! to keep every source file under the 350-line cap.

use crate::codegen::shape::{MatchContext, ShapeError, VarSeg};
use crate::codegen::shape_args::{match_fn_ptr, match_qself, match_segments};
use crate::preprocess::varseg::{is_varseg_type, varseg_prefix};
use quote::ToTokens;
/// A bare single-segment path with no generic args (`T` / `Vec`).
pub(crate) fn is_bare_ident(tp: &syn::TypePath) -> bool {
    tp.qself.is_none()
        && tp.path.segments.len() == 1
        && tp.path.segments.first().is_some_and(|s| matches!(s.arguments, syn::PathArguments::None))
}

/// The ident of a bare single-segment path expression (`N` in `[T; N]`);
/// `None` for any other expression (literals, arithmetic, `N + 1`, ...).
pub(crate) fn bare_path_ident(expr: &syn::Expr) -> Option<String> {
    let syn::Expr::Path(ep) = expr else { return None };
    if ep.qself.is_some() || ep.path.segments.len() != 1 {
        return None;
    }
    let seg = ep.path.segments.first()?;
    matches!(seg.arguments, syn::PathArguments::None).then(|| seg.ident.to_string())
}

/// Recursive position-by-position match (see module docs for the rules).
pub(crate) fn match_ty(
    template: &syn::Type, leaf: &syn::Type, context: &mut MatchContext<'_>,
) -> Result<(), ShapeError> {
    match template {
        // Bare ident: `_` is a wildcard (matches any type, never binds a
        // slot); an equal leaf ident → literal; anything else → slot bound
        // to the whole leaf subtree (the "0-arity → T := leaf" rule).
        syn::Type::Path(tp) if is_bare_ident(tp) => {
            let Some(seg) = tp.path.segments.first() else {
                return Err(ShapeError::ShapeMismatch(
                    "the bare-path template has no segment".into(),
                ));
            };
            let name = &seg.ident;
            if name == "_" {
                return Ok(());
            }
            if let syn::Type::Path(lp) = leaf
                && is_bare_ident(lp)
                && lp.path.segments.first().is_some_and(|s| s.ident == *name)
            {
                return context.map.keep(&name.to_string(), leaf.to_token_stream());
            }
            context.map.bind(&name.to_string(), leaf.to_token_stream())
        }
        // Composite path: structural compare + recurse into segments/args.
        syn::Type::Path(tp) => {
            let syn::Type::Path(lp) = leaf else {
                return Err(ShapeError::ShapeMismatch(
                    "the template is a path but the target is not".into(),
                ));
            };
            // Qualified paths recurse into the projection type and associated
            // segments. The `as Trait` path compares token-by-token; a qself on
            // one side only is a mismatch.
            if tp.qself.is_some() || lp.qself.is_some() {
                return match_qself(tp, lp, template, leaf, context);
            }
            match_segments(&tp.path, &lp.path, 0, 0, template, context)
        }
        // Structural containers: recurse into the element(s).
        syn::Type::Reference(t) => {
            let syn::Type::Reference(l) = leaf else {
                return Err(ShapeError::ShapeMismatch(
                    "the template is a reference but the target is not".into(),
                ));
            };
            if t.mutability.is_some() != l.mutability.is_some() {
                return Err(ShapeError::ShapeMismatch("reference mutability differs".into()));
            }
            if context.exact_ref_lifetimes
                && t.lifetime.to_token_stream().to_string()
                    != l.lifetime.to_token_stream().to_string()
            {
                return Err(ShapeError::ShapeMismatch(
                    "function reference lifetimes differ".into(),
                ));
            }
            match_ty(&t.elem, &l.elem, context)
        }
        syn::Type::Tuple(t) => {
            let syn::Type::Tuple(l) = leaf else {
                return Err(ShapeError::ShapeMismatch(
                    "the template is a tuple but the target is not".into(),
                ));
            };
            // Variadic segments (`ident@..` placeholders): the remaining
            // leaf positions (after the fixed template elements) split
            // evenly across the segments. Each segment binds its name
            // sequence (`prefix` + leaf start index..) to the corresponding
            // leaf elements — name numbering aligns with the leaf position
            // (user-confirmed: `(A, B@..)` → `B1, B2, ...`).
            let seg_count = t.elems.iter().filter(|e| is_varseg_type(e)).count();
            if seg_count > 0 {
                let fixed = t.elems.len() - seg_count;
                if l.elems.len() < fixed {
                    return Err(ShapeError::ShapeMismatch(format!(
                        "tuple arity differs (template has {} fixed elements, target has {})",
                        fixed,
                        l.elems.len(),
                    )));
                }
                let remaining = l.elems.len() - fixed;
                if remaining % seg_count != 0 {
                    return Err(ShapeError::ShapeMismatch(format!(
                        "variadic segments cannot be split evenly: target tuple has {} \
                         elements after {} fixed, split across {} segments",
                        remaining, fixed, seg_count,
                    )));
                }
                let seg_len = remaining / seg_count;
                let mut leaf_idx = 0;
                for te in &t.elems {
                    if is_varseg_type(te) {
                        let Some(prefix) = varseg_prefix(te) else {
                            return Err(ShapeError::ShapeMismatch(
                                "malformed variadic segment marker".into(),
                            ));
                        };
                        if context.segs.iter().any(|s| s.prefix == prefix) {
                            return Err(ShapeError::ShapeMismatch(format!(
                                "duplicate variadic segment prefix `{}` (each \
                                 `ident@..` in one template must be unique)",
                                prefix,
                            )));
                        }
                        context.segs.push(VarSeg {
                            prefix: prefix.clone(),
                            start: leaf_idx,
                            len: seg_len,
                        });
                        for k in 0..seg_len {
                            // Structured binding: (prefix, leaf position) —
                            // the repeat-block substitution splices the
                            // bound element directly (no minted name). Read
                            // through the iterator: the arity check above
                            // guarantees the element exists.
                            let Some(elem) = l.elems.iter().nth(leaf_idx + k) else {
                                return Err(ShapeError::ShapeMismatch(
                                    "variadic segment indexes past the target tuple".into(),
                                ));
                            };
                            context.map.bind_seg(&prefix, leaf_idx + k, elem.to_token_stream())?;
                        }
                        leaf_idx += seg_len;
                    } else {
                        let Some(leaf_elem) = l.elems.iter().nth(leaf_idx) else {
                            return Err(ShapeError::ShapeMismatch(
                                "the template has more elements than the target tuple".into(),
                            ));
                        };
                        match_ty(te, leaf_elem, context)?;
                        leaf_idx += 1;
                    }
                }
                return Ok(());
            }
            if t.elems.len() != l.elems.len() {
                return Err(ShapeError::ShapeMismatch(format!(
                    "tuple arity differs (template has {}, target has {})",
                    t.elems.len(),
                    l.elems.len(),
                )));
            }
            for (te, le) in t.elems.iter().zip(l.elems.iter()) {
                match_ty(te, le, context)?;
            }
            Ok(())
        }
        syn::Type::Array(t) => {
            // A variadic-segment marker (`[A; ()]`) in a **generic-argument
            // position** (`A<(T@..)>` against `Box<(P0, P1)>`): the leaf arg
            // must be a tuple whose elements the segment binds (`T0 := P0`,
            // `T1 := P1`). The tuple-element case is handled by the tuple
            // arm below; here the marker sits inside a path's `<...>`.
            if crate::preprocess::varseg::is_varseg_array(t) {
                let syn::Type::Tuple(tup) = leaf else {
                    return Err(ShapeError::ShapeMismatch(
                        "a variadic segment (`ident@..`) in a generic argument \
                         needs a tuple target (`A<(T@..)>` against `A<(P0, P1)>`)"
                            .into(),
                    ));
                };
                let Some(prefix) =
                    crate::preprocess::varseg::varseg_prefix(&syn::Type::Array(t.clone()))
                else {
                    return Err(ShapeError::ShapeMismatch(
                        "malformed variadic segment marker".into(),
                    ));
                };
                for (k, elem) in tup.elems.iter().enumerate() {
                    context.map.bind_seg(&prefix, k, elem.to_token_stream())?;
                }
                return Ok(());
            }
            let syn::Type::Array(l) = leaf else {
                // A variadic-segment marker (`[A; ()]`) never reaches this
                // arm — the generic-argument case above handled it (with the
                // tuple-target requirement); here the template is a plain
                // array and the target is not one.
                return Err(ShapeError::ShapeMismatch(
                    "the template is an array but the target is not".into(),
                ));
            };
            // Length: `_` is a wildcard (matches any length, never binds);
            // a bare const-param name in the template (`[A; N]`) is a slot
            // bound to the leaf's length expression (any literal / const
            // generic); anything else compares verbatim (`[A; 3]` ↔
            // `[u8; 3]`).
            if matches!(t.len, syn::Expr::Infer(_)) {
                // `_` wildcard
            } else if let Some(name) = bare_path_ident(&t.len) {
                if name != "_" {
                    if name == l.len.to_token_stream().to_string() {
                        context.map.keep(&name, l.len.to_token_stream())?;
                    } else {
                        context.map.bind(&name, l.len.to_token_stream())?;
                    }
                }
            } else if t.len.to_token_stream().to_string() != l.len.to_token_stream().to_string() {
                return Err(ShapeError::ShapeMismatch("array length differs".into()));
            }
            match_ty(&t.elem, &l.elem, context)
        }
        syn::Type::Slice(t) => {
            let syn::Type::Slice(l) = leaf else {
                return Err(ShapeError::ShapeMismatch(
                    "the template is a slice but the target is not".into(),
                ));
            };
            match_ty(&t.elem, &l.elem, context)
        }
        syn::Type::Ptr(t) => {
            let syn::Type::Ptr(l) = leaf else {
                return Err(ShapeError::ShapeMismatch(
                    "the template is a pointer but the target is not".into(),
                ));
            };
            // syn 3 `PointerMutability` has no `PartialEq` — compare by arm.
            let mut_eq = matches!(
                (&t.mutability, &l.mutability),
                (syn::PointerMutability::Const(_), syn::PointerMutability::Const(_))
                    | (syn::PointerMutability::Mut(_), syn::PointerMutability::Mut(_))
            );
            if !mut_eq {
                return Err(ShapeError::ShapeMismatch("pointer mutability differs".into()));
            }
            match_ty(&t.elem, &l.elem, context)
        }
        syn::Type::Paren(t) => {
            let syn::Type::Paren(l) = leaf else {
                return Err(ShapeError::ShapeMismatch(
                    "the template is a parenthesized type but the target is not".into(),
                ));
            };
            match_ty(&t.elem, &l.elem, context)
        }
        syn::Type::Group(t) => {
            let syn::Type::Group(l) = leaf else {
                return Err(ShapeError::ShapeMismatch(
                    "the template is a grouped type but the target is not".into(),
                ));
            };
            match_ty(&t.elem, &l.elem, context)
        }
        syn::Type::FnPtr(t) => {
            let syn::Type::FnPtr(l) = leaf else {
                return Err(ShapeError::ShapeMismatch(
                    "the template is a function pointer but the target is not".into(),
                ));
            };
            match_fn_ptr(t, l, context)
        }
        // `_` infer wildcard: matches ANY type, never binds a slot
        syn::Type::Infer(_) => Ok(()),
        // Everything else (trait objects, macros...):
        // verbatim compare — these forms do not expose binding positions.
        other => {
            if other.to_token_stream().to_string() != leaf.to_token_stream().to_string() {
                return Err(ShapeError::ShapeMismatch(format!(
                    "template `{}` does not match target `{}`",
                    other.to_token_stream(),
                    leaf.to_token_stream(),
                )));
            }
            Ok(())
        }
    }
}
