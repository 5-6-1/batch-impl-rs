//! The shape-matching kernel shared by Ext 1 (`#[batch_impl]` ItemImpl
//! entry) and Ext 2 (`impl{...}` Self-part shape binding on the trait
//! entries): matches a shape template (`syn::Type`) against a leaf type
//! (a matrix leaf / the for-Type) and produces the slot mapping.
//!
//! Binding semantics (user-confirmed): the template and the leaf are
//! compared **position by position** — an ident that is **equal** to the
//! leaf's ident at that position is a literal (not bound, not replaced);
//! an ident that **differs** is a binding slot, mapped to the leaf's
//! subtree at that position. Composite nodes compare structurally (generic
//! arity, nesting, separators, path segment count — no path normalization);
//! non-ident tokens must be equal verbatim.

use proc_macro2::{TokenStream, TokenTree};
use quote::ToTokens;

use crate::preprocess::varseg::{is_varseg_type, varseg_prefix};

/// A variadic segment (`ident@..`) resolved by a shape match: the name
/// prefix, the leaf start index (= the name numbering start), and the
/// element count. Collected in template order; duplicate prefixes are
/// rejected by the match.
#[derive(Clone, Debug)]
pub(crate) struct VarSeg {
    pub(crate) prefix: String,
    pub(crate) start: usize,
    pub(crate) len: usize,
}

/// The slot mapping produced by a shape match: slot name → bound leaf
/// subtree. Order-preserving (rendering walks it in match order).
#[derive(Default)]
pub(crate) struct Mapping {
    slots: Vec<(String, TokenStream)>,
}

impl Mapping {
    /// Binds `name` to `value`, rejecting an inconsistent re-binding
    /// (the same slot mapped to a different subtree — no override).
    fn bind(&mut self, name: &str, value: TokenStream) -> Result<(), ShapeError> {
        if let Some((_, old)) = self.slots.iter().find(|(n, _)| n == name) {
            if old.to_string() != value.to_string() {
                return Err(ShapeError::InconsistentBinding(name.to_string(), old.clone(), value));
            }
            // Redundant but identical re-binding: keep (legal).
            return Ok(());
        }
        self.slots.push((name.to_string(), value));
        Ok(())
    }

    /// The slot entries (slot name, bound value), in match order.
    pub(crate) fn entries(&self) -> &[(String, TokenStream)] {
        &self.slots
    }

    /// Merges another mapping into this one; a conflicting re-binding of the
    /// same slot errors (`InconsistentBinding`), identical ones are kept.
    pub(crate) fn merge(&mut self, other: Mapping) -> Result<(), ShapeError> {
        for (name, value) in other.slots {
            self.bind(&name, value)?;
        }
        Ok(())
    }
}

/// Shape-match failure: the template cannot destructure the leaf.
#[derive(Debug)]
pub(crate) enum ShapeError {
    /// Structural or verbatim mismatch (template vs leaf shapes differ).
    ShapeMismatch(String),
    /// The same slot bound to two different subtrees across merged
    /// templates (`impl{...} impl{...}`).
    InconsistentBinding(String, TokenStream, TokenStream),
}

impl ShapeError {
    /// User-language diagnostic for the error.
    pub(crate) fn message(&self) -> String {
        match self {
            ShapeError::ShapeMismatch(why) => {
                format!(
                    "batch-impl: `impl{{...}}` template cannot destructure the target type ({why})"
                )
            }
            ShapeError::InconsistentBinding(name, old, new) => format!(
                "batch-impl: binding slot `{}` is bound to different subtrees \
                 across merged `impl{{...}}` templates (`{}` vs `{}`)",
                name, old, new
            ),
        }
    }
}

/// Matches `template` against `leaf`, producing the slot mapping and the
/// resolved variadic segments (`ident@..`, in template order).
pub(crate) fn match_shape(
    template: &syn::Type, leaf: &syn::Type,
) -> Result<(Mapping, Vec<VarSeg>), ShapeError> {
    let mut map = Mapping::default();
    let mut segs = vec![];
    match_ty(template, leaf, &mut map, &mut segs)?;
    Ok((map, segs))
}

/// Rewrites a token stream, replacing every ident equal to a slot name with
/// the slot's bound value. Recursive (groups descended) — the same shape the
/// slot rewrites need for the target type / where predicates / body.
pub(crate) fn apply_mapping(tokens: TokenStream, entries: &[(String, TokenStream)]) -> TokenStream {
    tokens
        .into_iter()
        .flat_map(|tt| match tt {
            TokenTree::Ident(id) => {
                let s = id.to_string();
                match entries.iter().find(|(name, _)| name.as_str() == s) {
                    Some((_, repl)) => repl.clone(),
                    None => TokenStream::from(TokenTree::Ident(id)),
                }
            }
            TokenTree::Group(g) => {
                let inner = apply_mapping(g.stream(), entries);
                let mut ng = proc_macro2::Group::new(g.delimiter(), inner);
                ng.set_span(g.span());
                TokenStream::from(TokenTree::Group(ng))
            }
            other => TokenStream::from(other),
        })
        .collect()
}

/// A bare single-segment path with no generic args (`T` / `Vec`).
fn is_bare_ident(tp: &syn::TypePath) -> bool {
    tp.qself.is_none()
        && tp.path.segments.len() == 1
        && matches!(tp.path.segments[0].arguments, syn::PathArguments::None)
}

/// The ident of a variadic-segment placeholder type (defensive: the caller
/// has already checked `is_varseg_type`; `None` keeps the no-panic promise
/// on any internal drift).
fn varseg_ident(tp: &syn::Type) -> Option<&syn::Ident> {
    let syn::Type::Path(p) = tp else { return None };
    (p.path.segments.len() == 1).then(|| &p.path.segments[0].ident)
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
fn match_ty(
    template: &syn::Type, leaf: &syn::Type, map: &mut Mapping, segs: &mut Vec<VarSeg>,
) -> Result<(), ShapeError> {
    match template {
        // Bare ident: equal leaf ident → literal; anything else → slot
        // bound to the whole leaf subtree (the "0-arity → T := leaf" rule).
        // A variadic-segment placeholder is legal only as a tuple element —
        // reaching the bare-ident arm means it sits elsewhere (rejected).
        syn::Type::Path(tp) if is_bare_ident(tp) => {
            let name = &tp.path.segments[0].ident;
            if is_varseg_type(template) {
                return Err(ShapeError::ShapeMismatch(
                    "a variadic segment (`ident@..`) is only supported as a tuple element \
                     inside an `impl{...}` template"
                        .into(),
                ));
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
                        let Some(ident) = varseg_ident(te) else {
                            return Err(ShapeError::ShapeMismatch(
                                "malformed variadic segment placeholder".into(),
                            ));
                        };
                        let Some(prefix) = varseg_prefix(ident) else {
                            return Err(ShapeError::ShapeMismatch(
                                "malformed variadic segment placeholder".into(),
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
                            let name = format!("{}{}", prefix, leaf_idx + k);
                            map.bind(&name, l.elems[leaf_idx + k].to_token_stream())?;
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
            let syn::Type::Array(l) = leaf else {
                return Err(ShapeError::ShapeMismatch(
                    "the template is an array but the target is not".into(),
                ));
            };
            // Length: a bare const-param name in the template (`[A; N]`) is
            // a slot bound to the leaf's length expression (any literal /
            // const generic); anything else compares verbatim (`[A; 3]` ↔
            // `[u8; 3]`).
            if let Some(name) = bare_path_ident(&t.len) {
                map.bind(&name, l.len.to_token_stream())?;
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
