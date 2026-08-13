//! The fresh-generic naming protocol — the single source of truth for the
//! `_Param_*_BatchGen_` reserved pattern shared by three layers:
//!
//! - **generate** ([`fresh_param`]) — the apply layer mints group-position
//!   names `_Param_{g}_{i}_BatchGen_` (the codegen sweeper renumbers them);
//! - **construct** ([`at_ref_name`]) — the parse layer turns `@N` / `@g_i`
//!   position references into `_Param_{N}_BatchGen_` / `_Param_{g}_{i}_BatchGen_`;
//! - **parse** ([`parse_grouped_fresh`]) — the codegen sweeper and the where
//!   resolver identify and renumber the grouped form.
//!
//! Keeping the prefix/suffix constants here guarantees the three layers
//! cannot drift apart.

use proc_macro2::{Ident, TokenStream};
use quote::quote;

/// Reserved prefix of every macro-generated generic name.
pub(crate) const FRESH_PREFIX: &str = "_Param_";
/// Reserved suffix of every macro-generated generic name.
pub(crate) const FRESH_SUFFIX: &str = "_BatchGen_";

/// Generates a fresh generic param name `_Param_{g}_{i}_BatchGen_` (group g,
/// position i within the generator) that never collides with user code
/// (`_Param_*_BatchGen_` is a reserved pattern). The codegen sweeper
/// renumbers these to `_Param_0..N_BatchGen_` per impl before rendering.
pub(crate) fn fresh_param(g: usize, i: usize) -> TokenStream {
    let name = format!("{}{}_{}{}", FRESH_PREFIX, g, i, FRESH_SUFFIX);
    let ident = Ident::new(&name, proc_macro2::Span::call_site());
    quote!(#ident)
}

/// `@N` / `@g_i` position reference → the fresh name: `@0` → `_Param_0_BatchGen_`,
/// `@0_1` (a literal with an underscore) → `_Param_0_1_BatchGen_`; `None` for
/// anything else. The single-numbered form is a *reference* (constructed from
/// `@N`, kept through the sweep because it already matches the swept name);
/// the grouped form is renumbered by the sweeper along with the generated
/// names.
pub(crate) fn at_ref_name(lit: &str) -> Option<String> {
    if let Ok(n) = lit.parse::<usize>() {
        return Some(format!("{}{}{}", FRESH_PREFIX, n, FRESH_SUFFIX));
    }
    if let Some((g, i)) = lit.split_once('_')
        && let (Ok(g), Ok(i)) = (g.parse::<usize>(), i.parse::<usize>())
    {
        return Some(format!("{}{}_{}{}", FRESH_PREFIX, g, i, FRESH_SUFFIX));
    }
    None
}

/// Parses `_Param_{g}_{i}_BatchGen_`; returns `None` for any other ident
/// (including the single-numbered `_Param_{n}_BatchGen_` form constructed
/// from `@N` references).
pub(crate) fn parse_grouped_fresh(s: &str) -> Option<(usize, usize)> {
    let rest = s.strip_prefix(FRESH_PREFIX)?.strip_suffix(FRESH_SUFFIX)?;
    let (g, i) = rest.split_once('_')?;
    Some((g.parse().ok()?, i.parse().ok()?))
}

/// Parses the single-numbered `_Param_{n}_BatchGen_` form
/// (constructed from `@N` references); `None` for any other ident.
pub(crate) fn parse_numbered_fresh(s: &str) -> Option<usize> {
    let rest = s.strip_prefix(FRESH_PREFIX)?.strip_suffix(FRESH_SUFFIX)?;
    rest.parse().ok()
}

/// Whether an identifier matches the reserved fresh pattern
/// (`_Param_*_BatchGen_`, grouped or single-numbered).
pub(crate) fn is_fresh_name(s: &str) -> bool {
    s.starts_with(FRESH_PREFIX) && s.ends_with(FRESH_SUFFIX)
}
