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
/// for the external-trait scenario of `batch_impl_only`).
///
/// [`ConstCtx::Trait`] (`batch_trait!`): built-in + user table (leading
/// `@name=value;`).
#[derive(Clone, Copy)]
pub(crate) enum ConstCtx<'a> {
    Attribute {
        trait_def: &'a syn::ItemTrait,
        trait_full_path: &'a TokenStream,
    },
    Trait {
        user_table: &'a UserConsts,
    },
}

pub(crate) type UserConsts = HashMap<String, Vec<TokenTree>>;

impl<'a> ConstCtx<'a> {
    /// User constant table (only `batch_trait!` has one; attribute macros do
    /// not support custom definitions).
    pub(crate) fn user_table(&self) -> Option<&'a UserConsts> {
        match self {
            ConstCtx::Trait { user_table } => Some(user_table),
            ConstCtx::Attribute { .. } => None,
        }
    }

    /// Trait definition (only attribute macro entries have one;
    /// `batch_trait!` is a function-like macro and cannot get it).
    pub(crate) fn trait_def(&self) -> Option<&'a syn::ItemTrait> {
        match self {
            ConstCtx::Attribute { trait_def, .. } => Some(trait_def),
            ConstCtx::Trait { .. } => None,
        }
    }

    /// Full trait path (`batch_impl` = local name; `batch_impl_only` =
    /// external path).
    pub(crate) fn trait_full_path(&self) -> Option<&'a TokenStream> {
        match self {
            ConstCtx::Attribute { trait_full_path, .. } => Some(trait_full_path),
            ConstCtx::Trait { .. } => None,
        }
    }
}
