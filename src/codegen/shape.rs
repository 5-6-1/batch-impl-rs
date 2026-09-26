//! The shape-matching kernel shared by the impl entry (`#[batch_impl]` ItemImpl
//! entry) and the shape templates (`impl{...}` shape binding on the trait
//! entries): matches a shape template (`syn::Type`) against a matrix leaf
//! and produces the slot mapping. The leaf is already final; an impl entry
//! uses the mapping to rewrite its separate prototype type once.
//!
//! Binding semantics (user-confirmed): the template and the leaf are
//! compared **position by position** — an ident that is **equal** to the
//! leaf's ident at that position is a literal (not bound, not replaced);
//! an ident that **differs** is a binding slot, mapped to the leaf's
//! subtree at that position. Composite nodes compare structurally (generic
//! arity, nesting, path segment count — no path normalization). Function
//! pointers recurse into parameter/return types while checking their qualifiers;
//! parameter labels are not slots. Const argument kinds come from declarations.

use proc_macro2::{TokenStream, TokenTree};
use std::collections::HashSet;

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
///   `impl{W<T>}`, the fixed tuple element `T` in `impl{(T, A@..)}`);
///   rewritten in where/body via [`apply_mapping`], and in an impl entry's
///   prototype type/trait arguments via [`apply_type_mapping`]. An attribute
///   entry's already-resolved target leaf is never rewritten.
/// - `segs` — **variadic-segment elements** (`A@..`), keyed by the
///   structured `(prefix, leaf position)` pair; consumed directly by the
///   repeat-block substitution (`repeat_drivers.rs::substitute`, which
///   splices each round's element into the body), never as a bare name.
///
/// Both are order-preserving (rendering walks them in match order). The separate
/// `literals` list constrains matching and merging without becoming a replacement
/// slot or causing an unchanged generic declaration to be removed.
#[derive(Default)]
pub(crate) struct Mapping {
    slots: Vec<(String, TokenStream)>,
    // Equal positions constrain subsequent matches without becoming replacement
    // slots: identity mappings must never remove an impl's generic declaration.
    literals: Vec<(String, TokenStream)>,
    segs: Vec<((String, usize), TokenStream)>,
}

impl Mapping {
    /// Binds `name` to `value`, rejecting an inconsistent re-binding
    /// (the same slot mapped to a different subtree — no override).
    pub(crate) fn bind(&mut self, name: &str, value: TokenStream) -> Result<(), ShapeError> {
        if let Some((_, literal)) = self.literals.iter().find(|(n, _)| n == name)
            && literal.to_string() != value.to_string()
        {
            return Err(ShapeError::InconsistentBinding(name.into(), literal.clone(), value));
        }
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

    /// Records a same-position literal, checking bindings from earlier positions
    /// or merged templates. It is a constraint, never a replacement slot.
    pub(crate) fn keep(&mut self, name: &str, value: TokenStream) -> Result<(), ShapeError> {
        if let Some((_, bound)) = self.slots.iter().find(|(n, _)| n == name)
            && bound.to_string() != value.to_string()
        {
            return Err(ShapeError::InconsistentBinding(name.into(), bound.clone(), value));
        }
        if !self.literals.iter().any(|(n, _)| n == name) {
            self.literals.push((name.into(), value));
        }
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

    /// Restricts rewriting to selected declared names. Literal constraints belong
    /// to matching and are not needed by this substitution-only view.
    pub(crate) fn select_slots(&self, names: &HashSet<String>) -> Self {
        Self {
            slots: self.slots.iter().filter(|(name, _)| names.contains(name)).cloned().collect(),
            ..Self::default()
        }
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
        for (name, value) in other.literals {
            self.keep(&name, value)?;
        }
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
    template: &syn::Type, leaf: &syn::Type, declared_consts: &HashSet<String>,
) -> Result<(Mapping, Vec<VarSeg>), ShapeError> {
    let mut context = MatchContext {
        map: Mapping::default(),
        segs: Vec::new(),
        declared_consts,
        exact_ref_lifetimes: false,
    };
    crate::codegen::match_ty::match_ty(template, leaf, &mut context)?;
    Ok((context.map, context.segs))
}

/// Information shared by recursive shape comparisons. Declaration kinds come
/// from parsed generics; no spelling heuristic classifies a const argument.
pub(crate) struct MatchContext<'a> {
    pub(crate) map: Mapping,
    pub(crate) segs: Vec<VarSeg>,
    pub(crate) declared_consts: &'a HashSet<String>,
    pub(crate) exact_ref_lifetimes: bool,
}

/// Rewrites a token stream through the mapping: a bare ident equal to a
/// **user slot** name is replaced by the bound subtree (the user wrote that
/// name to be substituted — a fixed element like `T` in
/// `impl{(T, A@..)}`, or any other template ident). Segment elements are
/// NOT resolved here — the repeat-block expansion splices their values
/// directly (`repeat_drivers.rs::substitute` against [`Mapping::seg_value`]),
/// so no segment spelling ever reaches the body. Recursive (groups descended).
///
/// A **lifetime is not a slot position**: `'a` is a quote plus an ident, but
/// nothing ever binds a lifetime (the shape kernel compares named lifetimes
/// verbatim — see `match_ty`), so substituting that ident could only turn `'a`
/// into `'u8` for a slot *named* `a` and leave the impl without its lifetime
/// (measured: `E0261: use of undeclared lifetime name 'u8`). Both tokens pass
/// through verbatim.
pub(crate) fn apply_mapping(tokens: TokenStream, map: &Mapping) -> TokenStream {
    apply_mapping_inner(tokens, map, false)
}

/// Rewrites type-position tokens while preserving function parameter labels.
/// Used for impl prototype types/trait arguments and bound consts' type surfaces.
/// Labels are optional Rust documentation, not type slots; body rewriting retains
/// its existing lexical contract through [`apply_mapping`].
pub(crate) fn apply_type_mapping(tokens: TokenStream, map: &Mapping) -> TokenStream {
    apply_mapping_inner(tokens, map, true)
}

fn apply_mapping_inner(tokens: TokenStream, map: &Mapping, type_position: bool) -> TokenStream {
    let v = tokens.into_iter().collect::<Vec<_>>();
    let mut out = Vec::with_capacity(v.len());
    let mut i = 0;
    while let Some(t) = v.get(i) {
        match t {
            TokenTree::Punct(p) if p.as_char() == '\'' => {
                out.extend(crate::util::slice_window(&v, i, 2).to_vec());
                i += 2;
            }
            TokenTree::Ident(id) => {
                let s = id.to_string();
                // User-written fixed slots (`W`, `T`, `A0`).
                match map.slots.iter().find(|(name, _)| name.as_str() == s) {
                    Some((_, repl)) => out.extend(repl.clone()),
                    None => out.push(TokenTree::Ident(id.clone())),
                }
                i += 1;
            }
            TokenTree::Group(g) => {
                let fn_inputs = type_position
                    && g.delimiter() == proc_macro2::Delimiter::Parenthesis
                    && i.checked_sub(1)
                        .and_then(|p| v.get(p))
                        .is_some_and(|t| matches!(t, TokenTree::Ident(id) if id == "fn"));
                let inner = if fn_inputs {
                    map_fn_inputs(g.stream(), map)
                        .unwrap_or_else(|| apply_mapping_inner(g.stream(), map, type_position))
                } else {
                    apply_mapping_inner(g.stream(), map, type_position)
                };
                let mut ng = proc_macro2::Group::new(g.delimiter(), inner);
                ng.set_span(g.span());
                out.push(TokenTree::Group(ng));
                i += 1;
            }
            other => {
                out.push(other.clone());
                i += 1;
            }
        }
    }
    out.into_iter().collect()
}

fn map_fn_inputs(tokens: TokenStream, map: &Mapping) -> Option<TokenStream> {
    use quote::{ToTokens, quote};
    let syn::Type::FnPtr(function) = syn::parse2(quote!(fn(#tokens))).ok()? else { return None };
    let mut result = TokenStream::new();
    for pair in function.inputs.pairs() {
        let (arg, comma) = pair.into_tuple();
        let attrs = &arg.attrs;
        let name = arg.name.as_ref().map(|(name, colon)| quote!(#name #colon));
        let ty = apply_type_mapping(arg.ty.to_token_stream(), map);
        result.extend(quote!(#(#attrs)* #name #ty #comma));
    }
    function.variadic.to_tokens(&mut result);
    Some(result)
}
