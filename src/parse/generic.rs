//! Generic and angle-bracket parsing module.
//!
//! Provides matching and parsing of `<...>` generic parameters plus related helpers.

use proc_macro2::{Ident, TokenStream, TokenTree};
use quote::quote;

use crate::ast::*;
use crate::parse::parse_item;
use crate::util::{Cursor, is_single_colon, scan_stop};

// ============================================================
// Angle brackets and generic parameters
// ============================================================

/// Find the angle-bracket group after base (`delimiter![<>]`, produced by `angle_collect` pairing),
/// returning (base, args, rest). base must not be empty (empty = a type-parameter list,
/// handled by [`parse_type_params`]).
pub(crate) fn parse_generic(
    tokens: &[TokenTree],
) -> Option<(Vec<TokenTree>, TokenStream, Vec<TokenTree>)> {
    for (i, token) in tokens.iter().enumerate() {
        if let TokenTree::Group(g) = token
            && g.delimiter() == delimiter![<>]
        {
            if i == 0 {
                return None;
            }
            return Some((
                tokens[..i].to_vec(),
                g.stream(),
                tokens[i + 1..].to_vec(),
            ));
        }
    }
    None
}

/// Parse a bare type-parameter list that starts with an angle-bracket group (`<'a, T: Clone>`).
pub(crate) fn parse_type_params(
    tokens: &[TokenTree],
) -> Option<(TokenStream, Vec<TokenTree>)> {
    let TokenTree::Group(g) = tokens.first()? else {
        return None;
    };
    if g.delimiter() != delimiter![<>] {
        return None;
    }
    Some((g.stream(), tokens[1..].to_vec()))
}

/// Whether base ends with trait_name's ident (distinguishes `TraitName<T>` from plain generics)
pub(crate) fn is_trait_base(base: &[TokenTree], trait_name: Option<&Ident>) -> bool {
    trait_name.is_some_and(
        |name| matches!(base.last(), Some(TokenTree::Ident(last)) if last == name),
    )
}

/// Split by separator (angle brackets are already paired into opaque groups, so flat split)
///
/// **Note**: a flat `<A, B>` would be wrongly cut by a depth-0 comma split (`T: Two<A, B>` →
/// `T: Two<A` / `B>`); if the macro ever generates generic-group contents containing angle
/// brackets, they must be paired (`Group::new(delimiter![<>], ...)`) before insertion,
/// never scattered as flat `<...>`.
fn split_at_depth0(tokens: &[TokenTree], separator: char) -> Vec<&[TokenTree]> {
    let mut chunks = vec![];
    let mut rest = tokens;
    while let Some(index) = scan_stop(rest, &[separator]) {
        chunks.push(&rest[..index]);
        rest = &rest[index + 1..];
    }
    chunks.push(rest);
    chunks
}

/// Find the first `:` that is not part of `::` (used to split `T: Bound`)
fn find_colon_at_depth0(tokens: &[TokenTree]) -> Option<usize> {
    scan_stop(tokens, &[':']).filter(|&index| is_single_colon(tokens, index))
}

/// Parse `<T: Clone, U, Item=V>` contents: parameter list + associated-type bindings
pub(crate) fn parse_angle_bracket_contents(
    tokens: &[TokenTree], trait_name: Option<&Ident>,
) -> TyTypeParam {
    let mut params = vec![];
    let mut bindings = vec![];
    for chunk in split_at_depth0(tokens, ',') {
        if chunk.is_empty() {
            continue;
        }
        if let Some(eq) = scan_stop(chunk, &['=']) {
            bindings.push((
                chunk[..eq].iter().cloned().collect(),
                chunk[eq + 1..].iter().cloned().collect(),
            ));
        } else if let Some(colon) = find_colon_at_depth0(chunk) {
            params.push((
                chunk[..colon].iter().cloned().collect(),
                parse_item(
                    &mut Cursor::new(&chunk[colon + 1..]),
                    Op::Dash,
                    trait_name,
                )
                .unwrap_or_else(empty)
                .into(),
            ));
        } else {
            params.push((chunk.iter().cloned().collect(), None));
        }
    }
    TyTypeParam { params, bindings }
}

// ============================================================
// Fallbacks
// ============================================================

/// Wrap a token sequence as a Primitive passthrough node (any unrecognized type lands here)
pub(crate) fn primitive(tokens: &[TokenTree]) -> Ty {
    TyPrimitive(tokens.iter().cloned().collect()).into()
}

/// Empty token node (fallback for unwrap_or_else)
pub(crate) fn empty() -> Ty {
    TyPrimitive(quote![]).into()
}
