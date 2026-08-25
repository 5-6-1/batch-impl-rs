//! The shape-matching kernel shared by the impl entry (`#[batch_impl]` ItemImpl
//! entry) and the shape templates (`impl{...}` shape binding on the trait
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

/// The slot mapping produced by a shape match. Two channels, split by who
/// wrote the name:
/// - `slots` — **user-written** fixed placeholder names (`W`, `T` in
///   `impl{W<T>}`); replaced wherever the ident appears (the documented
///   substitution semantics — the name is the user's own).
/// - `segs` — **variadic-segment elements** (`A@..`), keyed by the
///   structured `(prefix, leaf position)` pair and carried in token domains
///   as `@{prefix#pos}`; matched structurally, never as a bare name.
///
/// Both order-preserving (rendering walks them in match order).
#[derive(Default)]
pub(crate) struct Mapping {
    slots: Vec<(String, TokenStream)>,
    segs: Vec<((String, usize), TokenStream)>,
}

impl Mapping {
    /// Binds `name` to `value`, rejecting an inconsistent re-binding
    /// (the same slot mapped to a different subtree — no override).
    pub(crate) fn bind(&mut self, name: &str, value: TokenStream) -> Result<(), ShapeError> {
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

    /// Binds segment element `(prefix, pos)` to its leaf subtree; same
    /// re-binding rules as [`Mapping::bind`].
    pub(crate) fn bind_seg(
        &mut self, prefix: &str, pos: usize, value: TokenStream,
    ) -> Result<(), ShapeError> {
        if let Some(entry) = self.segs.iter_mut().find(|((p, k), _)| *p == prefix && *k == pos) {
            let old = entry.1.clone();
            if old.to_string() != value.to_string() {
                return Err(ShapeError::InconsistentBinding(
                    format!("{prefix}#{}", pos),
                    old,
                    value,
                ));
            }
            return Ok(());
        }
        self.segs.push(((prefix.to_string(), pos), value));
        Ok(())
    }

    /// The user-slot entries (slot name, bound value), in match order.
    pub(crate) fn slots(&self) -> &[(String, TokenStream)] {
        &self.slots
    }

    /// The segment-slot entries (`(prefix, position)`, bound value), in
    /// match order.
    pub(crate) fn seg_entries(&self) -> &[((String, usize), TokenStream)] {
        &self.segs
    }

    /// Merges another mapping into this one; a conflicting re-binding of
    /// the same slot errors (`InconsistentBinding`), identical ones are kept.
    pub(crate) fn merge(&mut self, other: Mapping) -> Result<(), ShapeError> {
        for (name, value) in other.slots {
            self.bind(&name, value)?;
        }
        for (key, value) in other.segs {
            self.bind_seg(&key.0, key.1, value)?;
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
    crate::codegen::match_ty::match_ty(template, leaf, &mut map, &mut segs)?;
    Ok((map, segs))
}

/// Rewrites a token stream through the mapping. Two matchers, one per
/// channel:
/// - a bare ident equal to a **user slot** name is replaced (the user wrote
///   that name to be substituted);
/// - an `@{prefix#pos}` **segment-slot carrier** (parsed as a SegRef,
///   matched by the structured `(prefix, position)` pair) is replaced by
///   the bound leaf subtree — the repeat-block expansion emits these
///   carriers, so no minted identifier ever needs a textual search.
///
/// Recursive (groups descended).
pub(crate) fn apply_mapping(tokens: TokenStream, map: &Mapping) -> TokenStream {
    let v: Vec<_> = tokens.into_iter().collect();
    let mut out: Vec<TokenTree> = Vec::with_capacity(v.len());
    let mut i = 0;
    while i < v.len() {
        // Segment-slot carrier: `@` + Brace group holding `prefix#pos`.
        if let (TokenTree::Punct(p), Some(TokenTree::Group(g))) = (&v[i], v.get(i + 1))
            && p.as_char() == '@'
            && g.delimiter() == proc_macro2::Delimiter::Brace
        {
            let inner: String =
                g.stream().into_iter().map(|t| t.to_string()).collect::<Vec<_>>().join("");
            if let Some(r) = crate::ast::fresh::SegRef::parse(&inner)
                && let Some(((_, _), repl)) =
                    map.segs.iter().find(|((p2, k2), _)| *p2 == r.prefix && *k2 == r.pos)
            {
                out.extend(repl.clone());
                i += 2;
                continue;
            }
            // Not a known segment slot: keep verbatim (validation reports
            // dangling references elsewhere).
            out.push(v[i].clone());
            out.push(v[i + 1].clone());
            i += 2;
            continue;
        }
        match &v[i] {
            TokenTree::Ident(id) => {
                let s = id.to_string();
                // User-written fixed slots (`W`, `T`).
                match map.slots.iter().find(|(name, _)| name.as_str() == s) {
                    Some((_, repl)) => out.extend(repl.clone()),
                    None => out.push(TokenTree::Ident(id.clone())),
                }
            }
            TokenTree::Group(g) => {
                let inner = apply_mapping(g.stream(), map);
                let mut ng = proc_macro2::Group::new(g.delimiter(), inner);
                ng.set_span(g.span());
                out.push(TokenTree::Group(ng));
            }
            other => out.push(other.clone()),
        }
        i += 1;
    }
    out.into_iter().collect()
}
