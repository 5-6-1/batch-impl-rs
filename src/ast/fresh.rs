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

/// Reserved infix of the `@N..` range placeholders: the open-range form is
/// `_Param_{N}_With_BatchGen_`, the closed form `_Param_{N}_With_{M}_BatchGen_`.
/// The `_With` infix keeps `parse_grouped_fresh` / `parse_numbered_fresh`
/// from ever matching these (`{N}_With` is not a number and `With` is not a
/// position), so the sweeper and dangling-reference validators cannot touch
/// them — they are recognized only by [`parse_range_fresh`].
pub(crate) const RANGE_WITH_INFIX: &str = "_With_";

/// A resolved `@N..` / `@N..M` range reference: `start` with an optional
/// inclusive `end` (`None` = open to the last fresh).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FreshRange {
    pub(crate) start: usize,
    pub(crate) end: Option<usize>,
}

/// The range placeholder name for `@N..` / `@N..M`: `_Param_{N}_With_BatchGen_`
/// (open) or `_Param_{N}_With_{M}_BatchGen_` (closed, `end` inclusive).
pub(crate) fn range_fresh_name(range: FreshRange) -> String {
    match range.end {
        Some(end) => {
            format!("{}{}{}{}{}", FRESH_PREFIX, range.start, RANGE_WITH_INFIX, end, FRESH_SUFFIX)
        }
        // open: `_With` (no end; the `_BatchGen_` suffix provides the tail `_`)
        None => format!("{}{}_With{}", FRESH_PREFIX, range.start, FRESH_SUFFIX),
    }
}

/// Parses a range placeholder ident (`_Param_N_With_BatchGen_` /
/// `_Param_N_With_M_BatchGen_`); `None` for anything else (including the
/// plain fresh forms — those belong to `parse_grouped_fresh` /
/// `parse_numbered_fresh`).
pub(crate) fn parse_range_fresh(s: &str) -> Option<FreshRange> {
    // `_Param_{N}_With[_M]_BatchGen_` → strip the fixed head/suffix, then
    // split the middle on the `_With_` / `_With` marker.
    let rest = s.strip_prefix(FRESH_PREFIX)?.strip_suffix(FRESH_SUFFIX)?;
    if let Some((start_str, tail)) = rest.split_once("_With_") {
        // closed: `N_With_M`
        let start = start_str.parse::<usize>().ok()?;
        let end = tail.parse::<usize>().ok()?;
        return (start <= end).then_some(FreshRange { start, end: Some(end) });
    }
    // open: `N_With` (no trailing `_` — the suffix already consumed it)
    let (start_str, marker) = rest.rsplit_once("_With")?;
    (marker.is_empty())
        .then_some(FreshRange { start: start_str.parse().ok()?, end: None })
}

/// Whether an identifier matches the reserved fresh pattern
/// (`_Param_*_BatchGen_`, grouped or single-numbered).
pub(crate) fn is_fresh_name(s: &str) -> bool {
    s.starts_with(FRESH_PREFIX) && s.ends_with(FRESH_SUFFIX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_placeholder_roundtrip() {
        for (range, expect) in [
            (FreshRange { start: 0, end: None }, "_Param_0_With_BatchGen_"),
            (FreshRange { start: 1, end: None }, "_Param_1_With_BatchGen_"),
            (FreshRange { start: 0, end: Some(2) }, "_Param_0_With_2_BatchGen_"),
            (FreshRange { start: 1, end: Some(3) }, "_Param_1_With_3_BatchGen_"),
        ] {
            let name = range_fresh_name(range);
            assert_eq!(name, expect);
            assert_eq!(parse_range_fresh(&name), Some(range), "{name}");
        }
    }

    #[test]
    fn range_placeholder_not_confused_with_plain_fresh() {
        // The `_With` infix must keep the sweeper's strict matchers away.
        for name in ["_Param_0_With_BatchGen_", "_Param_1_With_2_BatchGen_"] {
            assert_eq!(parse_grouped_fresh(name), None, "{name}");
            assert_eq!(parse_numbered_fresh(name), None, "{name}");
        }
        // And the plain forms are not range placeholders.
        for name in ["_Param_0_BatchGen_", "_Param_0_1_BatchGen_"] {
            assert_eq!(parse_range_fresh(name), None, "{name}");
        }
    }

    #[test]
    fn range_placeholder_invalid_forms() {
        assert_eq!(parse_range_fresh("_Param_x_With_BatchGen_"), None);
        assert_eq!(parse_range_fresh("_Param_2_With_1_BatchGen_"), None); // start > end
        assert_eq!(parse_range_fresh("_Param_0_With_x_BatchGen_"), None);
        assert_eq!(parse_range_fresh("plain"), None);
    }
}
