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
///   `impl{W<T>}`, the explicit segment start `A0` in `impl{(A0, @A..)}`);
///   replaced wherever the ident appears in where/body/target via
///   [`apply_mapping`] (the documented substitution semantics — the name is
///   the user's own).
/// - `segs` — **variadic-segment elements** (`A@..`), keyed by the
///   structured `(prefix, leaf position)` pair; consumed directly by the
///   repeat-block substitution (`repeat_drivers.rs::substitute`, which
///   splices each round's element into the body), never as a bare name.
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

    /// The bound leaf subtree of one segment element `(prefix, pos)` — the
    /// repeat-block substitution reads these directly and splices the value
    /// into the round's output (`@ident` → the i-th element's tokens).
    pub(crate) fn seg_value(&self, prefix: &str, pos: usize) -> Option<&TokenStream> {
        self.segs.iter().find(|((p, k), _)| p == prefix && *k == pos).map(|(_, v)| v)
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

/// Rewrites a token stream through the mapping: a bare ident equal to a
/// **user slot** name is replaced by the bound subtree (the user wrote that
/// name to be substituted — an explicit fixed element like `A0` in
/// `impl{(A0, @A..)}`, or any other template ident). Segment elements are
/// NOT resolved here — the repeat-block expansion splices their values
/// directly (`repeat_drivers.rs::substitute` against [`Mapping::seg_value`]),
/// so no segment spelling ever reaches the body. Recursive (groups descended).
pub(crate) fn apply_mapping(tokens: TokenStream, map: &Mapping) -> TokenStream {
    let v: Vec<_> = tokens.into_iter().collect();
    let mut out: Vec<TokenTree> = Vec::with_capacity(v.len());
    for t in v {
        match t {
            TokenTree::Ident(id) => {
                let s = id.to_string();
                // User-written fixed slots (`W`, `T`, `A0`).
                match map.slots.iter().find(|(name, _)| name.as_str() == s) {
                    Some((_, repl)) => out.extend(repl.clone()),
                    None => out.push(TokenTree::Ident(id)),
                }
            }
            TokenTree::Group(g) => {
                let inner = apply_mapping(g.stream(), map);
                let mut ng = proc_macro2::Group::new(g.delimiter(), inner);
                ng.set_span(g.span());
                out.push(TokenTree::Group(ng));
            }
            other => out.push(other),
        }
    }
    out.into_iter().collect()
}
