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

use proc_macro2::{Ident, TokenStream, TokenTree};
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

/// A resolved `@N` / `@g_i` / `@N..` / `@N..M` position reference — the
/// structured carrier that rides in the [`Ty`](crate::ast::Ty) tree
/// (`TyKind::Fresh`) and renders to the self-delimiting token form
/// `@{...}` (`@{0}`, `@{1_0..}`, `@{0..=3}`) for the token-level resolvers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FreshRef {
    /// `Some(L)` for the grouped forms (`@g_i` / `@L_N..` — within generator
    /// group L, stable across array dispatch); `None` is the flat form.
    pub(crate) group: Option<usize>,
    /// Flattened index or in-group position (numbered from 0).
    pub(crate) start: usize,
    pub(crate) end: FreshEnd,
}

/// The extent of a [`FreshRef`]: a single position (`@N` / `@g_i`), an open
/// range to the last fresh (`@N..` / `@L_N..` — empty when `start` is past
/// the end), or a closed range (`@N..M` / `@N..=M` normalized to inclusive).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FreshEnd {
    Single,
    Open,
    Closed(usize),
}

impl FreshRef {
    /// Whether this reference re-opens into several names (a range form).
    pub(crate) fn is_range(&self) -> bool {
        !matches!(self.end, FreshEnd::Single)
    }

    /// The `@{...}` inner spelling (`0`, `1_0..`, `0..=3`) — shared by the
    /// token emitter and the parser so the two can never drift.
    pub(crate) fn spell(&self) -> String {
        let head = match self.group {
            Some(l) => format!("{l}_{}", self.start),
            None => format!("{}", self.start),
        };
        match self.end {
            FreshEnd::Single => head,
            FreshEnd::Open => format!("{head}.."),
            FreshEnd::Closed(e) => format!("{head}..={e}"),
        }
    }

    /// Parses the inner spelling of an `@{...}` group; `None` for anything
    /// else. The single authority for both directions of the carrier.
    pub(crate) fn parse(s: &str) -> Option<Self> {
        let (group, rest) = match s.split_once('_') {
            // A grouped head needs a following position part; a plain number
            // has none (`split_once` on `0..=3` would misread `0..=3` — check
            // the tail parses as digits before accepting the split).
            Some((l, tail))
                if tail.split(['.', '_']).next()?.parse::<usize>().is_ok() =>
            {
                (Some(l.parse::<usize>().ok()?), tail)
            }
            _ => (None, s),
        };
        if let Some((start, end)) = rest.split_once("..=") {
            let start = start.parse::<usize>().ok()?;
            let end = end.parse::<usize>().ok()?;
            (start <= end).then_some(FreshRef { group, start, end: FreshEnd::Closed(end) })
        } else if let Some(stripped) = rest.strip_suffix("..") {
            let start = stripped.parse::<usize>().ok()?;
            (!stripped.is_empty()).then_some(FreshRef { group, start, end: FreshEnd::Open })
        } else {
            Some(FreshRef { group, start: rest.parse::<usize>().ok()?, end: FreshEnd::Single })
        }
    }
}

/// Emits the self-delimiting carrier tokens of a reference — a `@` punct
/// followed by a Brace group holding [`FreshRef::spell`]. The group is an
/// atomic unit for every token walker, so the reference survives any pass
/// untouched and can only be consumed by the resolvers that match this shape.
pub(crate) fn fresh_ref_tokens(r: FreshRef, span: proc_macro2::Span) -> TokenStream {
    let mut ts = TokenStream::new();
    let mut at = proc_macro2::Punct::new('@', proc_macro2::Spacing::Alone);
    at.set_span(span);
    ts.extend(std::iter::once(TokenTree::Punct(at)));
    // The spelled inner is always a valid token sequence (digits /
    // underscore / `..=`); the default keeps the no-panic promise under
    // internal invariant drift.
    let inner: TokenStream = r.spell().parse().unwrap_or_default();
    let mut g = proc_macro2::Group::new(proc_macro2::Delimiter::Brace, inner);
    g.set_span(span);
    ts.extend(std::iter::once(TokenTree::Group(g)));
    ts
}
/// Whether an identifier matches the reserved fresh pattern
/// (`_Param_*_BatchGen_`, grouped or single-numbered).
pub(crate) fn is_fresh_name(s: &str) -> bool {
    s.starts_with(FRESH_PREFIX) && s.ends_with(FRESH_SUFFIX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::FreshEnd;

    #[test]
    fn fresh_ref_spell_parse_roundtrip() {
        for r in [
            FreshRef { group: None, start: 0, end: FreshEnd::Single },
            FreshRef { group: None, start: 1, end: FreshEnd::Open },
            FreshRef { group: None, start: 0, end: FreshEnd::Closed(2) },
            FreshRef { group: Some(0), start: 0, end: FreshEnd::Single },
            FreshRef { group: Some(1), start: 0, end: FreshEnd::Open },
            FreshRef { group: Some(1), start: 1, end: FreshEnd::Closed(3) },
        ] {
            assert_eq!(FreshRef::parse(&r.spell()), Some(r), "{}", r.spell());
        }
    }

    #[test]
    fn fresh_ref_invalid_forms() {
        for s in ["", "x", "0..x", "1_", "2..1", "0_1_2"] {
            assert_eq!(FreshRef::parse(s), None, "{s}");
        }
    }

    #[test]
    fn plain_fresh_declarations_still_parse() {
        // The declaration-side protocol (sweeper) is untouched.
        assert_eq!(parse_grouped_fresh("_Param_0_1_BatchGen_"), Some((0, 1)));
        assert_eq!(parse_numbered_fresh("_Param_0_BatchGen_"), Some(0));
    }
}