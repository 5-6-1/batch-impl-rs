//! Generic and angle-bracket parsing module.
//!
//! Provides matching and parsing of `<...>` generic parameters plus related helpers.

use proc_macro2::{Delimiter, Ident, TokenStream, TokenTree};
use quote::{ToTokens, quote};

use crate::ast::*;
use crate::parse::parse_item;
use crate::parse::resolve_at_refs;
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
pub(crate) fn split_at_depth0(
    tokens: &[TokenTree], separator: char,
) -> Vec<&[TokenTree]> {
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
        // Splat arg: `Foo<*(a,b)>` / `Foo<*[a,b]>` flattens into multiple
        // generic args (`Foo<a,b>`, one impl) — distinct from `Foo<[a,b]>`
        // which dispatches. A flattened generator's declaration cannot live
        // in a `TyTypeParam`, so it errors (the compile_error! token renders
        // in the impl header).
        if let [TokenTree::Punct(star), TokenTree::Group(g)] = chunk
            && star.as_char() == '*'
            && matches!(g.delimiter(), Delimiter::Bracket | Delimiter::Parenthesis)
        {
            let inner = g.stream().into_iter().collect::<Vec<TokenTree>>();
            let mut flat = vec![];
            let mut decl = None;
            for c in split_at_depth0(&inner, ',') {
                if c.is_empty() {
                    continue;
                }
                let (mut es, d) = splat_expand(
                    parse_item(&mut Cursor::new(c), Op::Dash, trait_name)
                        .unwrap_or_else(empty),
                );
                flat.append(&mut es);
                decl = merge_decls(decl, d);
            }
            if decl.is_some() {
                // No `;` here — a semicolon after `compile_error!` is illegal
                // inside a generic-arg list; the bare invocation still emits
                // the targeted error when rustc expands it in type position.
                let err_ident = Ident::new("compile_error", star.span());
                let err = quote! {
                    #err_ident!(
                        "batch-impl: a generator splat (`*(()^N)`) cannot be a \
                         generic argument (its fresh declaration has nowhere \
                         to live)"
                    )
                };
                params.push((err, None));
                continue;
            }
            for e in flat {
                let name = match resolve_at_refs(
                    &e.to_token_stream().into_iter().collect::<Vec<_>>(),
                ) {
                    Ok(v) => v.into_iter().collect(),
                    Err(e) => e,
                };
                params.push((name, None));
            }
            continue;
        }
        // `@N` position refs inside angle args (`Box<@0>`) are not parsed as
        // types (flat token splitting) — resolve them to fresh names here.
        // A resolution error yields a `compile_error!` token stream that
        // surfaces when the impl header is rendered.
        if let Some(eq) = scan_stop(chunk, &['=']) {
            let value = match resolve_at_refs(&chunk[eq + 1..]) {
                Ok(v) => v.into_iter().collect(),
                Err(e) => e,
            };
            bindings.push((chunk[..eq].iter().cloned().collect(), value));
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
            let name = match resolve_at_refs(chunk) {
                Ok(v) => v.into_iter().collect(),
                Err(e) => e,
            };
            params.push((name, None));
        }
    }
    TyTypeParam { params, bindings }
}

// ============================================================
// Fallbacks
// ============================================================

/// Wrap a token sequence as a Primitive passthrough node (any unrecognized type lands here)
pub(crate) fn primitive(tokens: &[TokenTree]) -> Ty {
    let span =
        tokens.first().map(|t| t.span()).unwrap_or_else(proc_macro2::Span::call_site);
    TyPrimitive(tokens.iter().cloned().collect()).to_ty().with_span(span)
}

/// Empty token node (fallback for unwrap_or_else)
pub(crate) fn empty() -> Ty {
    TyPrimitive(quote![]).to_ty()
}
