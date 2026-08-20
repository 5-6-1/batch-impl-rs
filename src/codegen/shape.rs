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

/// The slot mapping produced by a shape match: slot name → bound leaf
/// subtree. Order-preserving (rendering walks it in match order).
#[derive(Default)]
pub(crate) struct Mapping {
    slots: Vec<(String, TokenStream)>,
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
    crate::codegen::match_ty::match_ty(template, leaf, &mut map, &mut segs)?;
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
