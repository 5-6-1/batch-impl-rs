//! The recursive shape-matching kernel: match_ty compares a shape template
//! (syn::Type) against a leaf type position-by-position, binding differing
//! idents as slots and resolving variadic segments. Split out of shape.rs
//! to keep every source file under the 350-line cap.

use crate::codegen::shape::{Mapping, ShapeError, VarSeg};
use crate::preprocess::varseg::{is_varseg_type, varseg_prefix};
use quote::ToTokens;
/// A bare single-segment path with no generic args (`T` / `Vec`).
fn is_bare_ident(tp: &syn::TypePath) -> bool {
    tp.qself.is_none()
        && tp.path.segments.len() == 1
        && matches!(tp.path.segments[0].arguments, syn::PathArguments::None)
}

/// The ident of a bare single-segment path expression (`N` in `[T; N]`);
/// `None` for any other expression (literals, arithmetic, `N + 1`, ...).
fn bare_path_ident(expr: &syn::Expr) -> Option<String> {
    let syn::Expr::Path(ep) = expr else { return None };
    if ep.qself.is_some()
        || ep.path.segments.len() != 1
        || !matches!(ep.path.segments[0].arguments, syn::PathArguments::None)
    {
        return None;
    }
    Some(ep.path.segments[0].ident.to_string())
}

/// Recursive position-by-position match (see module docs for the rules).
pub(crate) fn match_ty(
    template: &syn::Type, leaf: &syn::Type, map: &mut Mapping, segs: &mut Vec<VarSeg>,
) -> Result<(), ShapeError> {
    match template {
        // Bare ident: `_` is a wildcard (matches any type, never binds a
        // slot); an equal leaf ident → literal; anything else → slot bound
        // to the whole leaf subtree (the "0-arity → T := leaf" rule).
        syn::Type::Path(tp) if is_bare_ident(tp) => {
            let name = &tp.path.segments[0].ident;
            if name == "_" {
                return Ok(());
            }
            if let syn::Type::Path(lp) = leaf
                && is_bare_ident(lp)
                && lp.path.segments[0].ident == *name
            {
                return Ok(());
            }
            map.bind(&name.to_string(), leaf.to_token_stream())
        }
        // Composite path: structural compare + recurse into segments/args.
        syn::Type::Path(tp) => {
            let syn::Type::Path(lp) = leaf else {
                return Err(ShapeError::ShapeMismatch(
                    "the template is a path but the target is not".into(),
                ));
            };
            if tp.qself.is_some() || lp.qself.is_some() {
                return Err(ShapeError::ShapeMismatch(
                    "qualified paths (`<T as Trait>::...`) are not supported in templates".into(),
                ));
            }
            if tp.path.segments.len() != lp.path.segments.len() {
                return Err(ShapeError::ShapeMismatch(format!(
                    "path segment count differs (template `{}` has {}, target has {})",
                    template.to_token_stream(),
                    tp.path.segments.len(),
                    lp.path.segments.len(),
                )));
            }
            for (tseg, lseg) in tp.path.segments.iter().zip(lp.path.segments.iter()) {
                // Segment ident: equal → literal; different → slot bound to
                // the target segment's base ident.
                if tseg.ident != lseg.ident {
                    map.bind(&tseg.ident.to_string(), lseg.ident.to_token_stream())?;
                }
                match (&tseg.arguments, &lseg.arguments) {
                    (syn::PathArguments::None, syn::PathArguments::None) => {}
                    (
                        syn::PathArguments::AngleBracketed(t),
                        syn::PathArguments::AngleBracketed(l),
                    ) => {
                        if t.args.len() != l.args.len() {
                            return Err(ShapeError::ShapeMismatch(format!(
                                "generic arity differs (template `{}` has {} args, target has {})",
                                template.to_token_stream(),
                                t.args.len(),
                                l.args.len(),
                            )));
                        }
                        for (ta, la) in t.args.iter().zip(l.args.iter()) {
                            match (ta, la) {
                                (
                                    syn::GenericArgument::Type(tt),
                                    syn::GenericArgument::Type(lt),
                                ) => match_ty(tt, lt, map, segs)?,
                                // Lifetime args: `'_` (anonymous) is a
                                // wildcard matching any lifetime (skip);
                                // named lifetimes compare verbatim (`'a` vs
                                // `'b` mismatches — cross-lifetime binding is
                                // out of scope).
                                (
                                    syn::GenericArgument::Lifetime(tl),
                                    syn::GenericArgument::Lifetime(ll),
                                ) => {
                                    if tl.ident != "_" && tl.ident != ll.ident {
                                        return Err(ShapeError::ShapeMismatch(format!(
                                            "generic argument differs (template `{}` vs target `{}`)",
                                            ta.to_token_stream(),
                                            la.to_token_stream(),
                                        )));
                                    }
                                }
                                _ => {
                                    // Binding names, const args, lifetime-vs-
                                    // type: verbatim compare (no slots
                                    // inside; cross-class binding is out of
                                    // scope).
                                    if ta.to_token_stream().to_string()
                                        != la.to_token_stream().to_string()
                                    {
                                        return Err(ShapeError::ShapeMismatch(format!(
                                            "generic argument differs (template `{}` vs target `{}`)",
                                            ta.to_token_stream(),
                                            la.to_token_stream(),
                                        )));
                                    }
                                }
                            }
                        }
                    }
                    (
                        syn::PathArguments::Parenthesized(t),
                        syn::PathArguments::Parenthesized(l),
                    ) => {
                        // Fn-trait sugar (`Fn(A) -> B`): verbatim compare
                        // (syn 3 models the inputs as named args; slots
                        // inside fn-trait sugar are out of scope).
                        if t.to_token_stream().to_string() != l.to_token_stream().to_string() {
                            return Err(ShapeError::ShapeMismatch(
                                "parenthesized generic arguments differ".into(),
                            ));
                        }
                    }
                    _ => {
                        return Err(ShapeError::ShapeMismatch(format!(
                            "generic argument shape differs at segment `{}`",
                            tseg.ident,
                        )));
                    }
                }
            }
            Ok(())
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
            match_ty(&t.elem, &l.elem, map, segs)
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
                        if segs.iter().any(|s| s.prefix == prefix) {
                            return Err(ShapeError::ShapeMismatch(format!(
                                "duplicate variadic segment prefix `{}` (each \
                                 `ident@..` in one template must be unique)",
                                prefix,
                            )));
                        }
                        segs.push(VarSeg { prefix: prefix.clone(), start: leaf_idx, len: seg_len });
                        for k in 0..seg_len {
                            // Structured binding: (prefix, leaf position) —
                            // the repeat-block substitution splices the
                            // bound element directly (no minted name).
                            map.bind_seg(
                                &prefix,
                                leaf_idx + k,
                                l.elems[leaf_idx + k].to_token_stream(),
                            )?;
                        }
                        leaf_idx += seg_len;
                    } else {
                        match_ty(te, &l.elems[leaf_idx], map, segs)?;
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
                match_ty(te, le, map, segs)?;
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
                    map.bind_seg(&prefix, k, elem.to_token_stream())?;
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
                    map.bind(&name, l.len.to_token_stream())?;
                }
            } else if t.len.to_token_stream().to_string() != l.len.to_token_stream().to_string() {
                return Err(ShapeError::ShapeMismatch("array length differs".into()));
            }
            match_ty(&t.elem, &l.elem, map, segs)
        }
        syn::Type::Slice(t) => {
            let syn::Type::Slice(l) = leaf else {
                return Err(ShapeError::ShapeMismatch(
                    "the template is a slice but the target is not".into(),
                ));
            };
            match_ty(&t.elem, &l.elem, map, segs)
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
            match_ty(&t.elem, &l.elem, map, segs)
        }
        syn::Type::Paren(t) => {
            let syn::Type::Paren(l) = leaf else {
                return Err(ShapeError::ShapeMismatch(
                    "the template is a parenthesized type but the target is not".into(),
                ));
            };
            match_ty(&t.elem, &l.elem, map, segs)
        }
        syn::Type::Group(t) => {
            let syn::Type::Group(l) = leaf else {
                return Err(ShapeError::ShapeMismatch(
                    "the template is a grouped type but the target is not".into(),
                ));
            };
            match_ty(&t.elem, &l.elem, map, segs)
        }
        // `_` infer wildcard: matches ANY type, never binds a slot
        syn::Type::Infer(_) => Ok(()),
        // Everything else (fn pointers, trait objects, infer, macros...):
        // verbatim compare — templates only bind idents in path/container
        // positions; anything else must be written out exactly.
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
