//! The recursive shape-matching kernel: match_ty compares a shape template
//! (syn::Type) against a leaf type position-by-position, binding differing
//! idents as slots and resolving variadic segments. Split out of `shape.rs`
//! so that file stayed reviewable.

use crate::codegen::shape::{MatchContext, ShapeError};
use crate::codegen::shape_args::{match_fn_ptr, match_qself, match_segments};
use crate::preprocess::varseg::varseg_pattern;
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
    // A marker outside a tuple element consumes a tuple-valued type slot,
    // such as the argument of `Wrap<T@..>`.
    if let Some(pattern) = varseg_pattern(template) {
        let syn::Type::Tuple(tuple) = leaf else {
            return Err(ShapeError::ShapeMismatch(
                "a variadic segment (`P@..`) needs a tuple target".into(),
            ));
        };
        return super::shape_segments::match_segment(pattern, tuple.elems.iter(), 0, context);
    }
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
            super::shape_segments::match_tuple(t, l, context)
        }
        syn::Type::Array(t) => {
            let syn::Type::Array(l) = leaf else {
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
