//! `@` constant source context (the sources `expand_consts` resolves in one
//! pass, unioned: built-in name families, range families, `@trait`/`@all`/
//! `@Cow`, and the user table).
//!
//! Built-in constants (`@u*`/`@scalar`/range families) work in both
//! contexts; trait-aware constants (`@trait`/`@all` family/`@Cow`) resolve
//! only in the attribute macro entry (`@trait` is kept as a segment marker in
//! `batch_trait!`).

use std::collections::HashMap;

use proc_macro2::{TokenStream, TokenTree};

/// Source context for `@` constants.
///
/// [`ConstCtx::Attribute`] (`#[batch_impl]`/`#[batch_impl_only]`): built-in +
/// trait-aware constants (`@trait`/`@all` family/`@Cow`; `trait_full_path`
/// lets `@trait` expand to the full path, i.e. the `#ext::Trait:` prefix path
/// for the external-trait scenario of `batch_impl_only`). Custom `@name=value;`
/// definitions are **not** supported here (the 0.7.2 feature was reverted in
/// 0.8.0 — a definition segment errors in [`try_expand_at`]).
///
/// [`ConstCtx::ItemImpl`] (`#[batch_impl(spec)] impl ...`): the built-in
/// families (`@u*` / `@num` / ranges) work; `@trait` expands to the impl's
/// own trait path (`None` on an inherent impl — an error); the trait-aware
/// selectors (`@all` family) and position refs (`@N`) are rejected (no trait
/// definition, no fresh-generic system on this entry).
///
/// [`ConstCtx::Trait`] (`batch_trait!`): built-in + user table (leading
/// `@name=value;`).
#[derive(Clone, Copy)]
pub(crate) enum ConstCtx<'a> {
    Attribute { trait_def: &'a syn::ItemTrait, trait_full_path: &'a TokenStream },
    ItemImpl { trait_path: Option<&'a TokenStream> },
    Trait { user_table: &'a UserConsts },
}

pub(crate) type UserConsts = HashMap<String, Vec<TokenTree>>;

impl<'a> ConstCtx<'a> {
    /// User constant table (`batch_trait!` only — attribute macros do not
    /// collect a leading `@name=value;` section).
    pub(crate) fn user_table(&self) -> Option<&'a UserConsts> {
        match self {
            &ConstCtx::Trait { user_table } => user_table.into(),
            ConstCtx::Attribute { .. } | ConstCtx::ItemImpl { .. } => None,
        }
    }

    /// Trait definition (only attribute macro entries have one;
    /// `batch_trait!` is a function-like macro and cannot get it).
    pub(crate) fn trait_def(&self) -> Option<&'a syn::ItemTrait> {
        match self {
            &ConstCtx::Attribute { trait_def, .. } => trait_def.into(),
            ConstCtx::ItemImpl { .. } | ConstCtx::Trait { .. } => None,
        }
    }

    /// Full trait path (`batch_impl` = local name; `batch_impl_only` =
    /// external path; `ItemImpl` = the impl's own trait path).
    pub(crate) fn trait_full_path(&self) -> Option<&'a TokenStream> {
        match self {
            &ConstCtx::Attribute { trait_full_path, .. } => trait_full_path.into(),
            &ConstCtx::ItemImpl { trait_path } => trait_path,
            ConstCtx::Trait { .. } => None,
        }
    }

    /// Whether this is the ItemImpl entry (its `@N` refs and `@all` selectors
    /// are rejected — no fresh-generic system / trait definition there).
    pub(crate) fn is_item_impl(&self) -> bool {
        matches!(self, ConstCtx::ItemImpl { .. })
    }
}
