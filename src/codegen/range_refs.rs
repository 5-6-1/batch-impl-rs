//! `@N..` / `@N..M` range-reference expansion at the token level: a range
//! placeholder ident (`_Param_N_With_BatchGen_` / `_Param_N_With_M_BatchGen_`)
//! in a rendered type expands into the impl's fresh names — one position
//! becomes several (`Wrapper<@0..>` → `Wrapper<P0, P1, P2>`). The parse layer
//! folds a range into a single placeholder ident (so it may appear anywhere a
//! single `@N` can); only here does it re-open into the real generic list.
//!
//! Where-predicate ranges are handled by `resolve_where_at` on the raw `@N..`
//! form (predicate-subject expansion); this file covers the **type positions**
//! (target type, trait args, impl-generic bounds) whose placeholders were
//! produced by `parse::resolve_at_refs`.

use proc_macro2::{Group, TokenStream, TokenTree};

use crate::ast::fresh::{FreshRange, parse_range_fresh};
use crate::ast::{MAX_EXPAND, parse_grouped_fresh};
use crate::util::compile_error_str;

/// Expands every `@N..` / `@N..M` range placeholder in `tokens` against the
/// impl's fresh names: `_Param_0_With_BatchGen_` → `P0, P1, P2, ...` (each
/// fresh name an element), `_Param_1_With_3_BatchGen_` → `P1, P2, P3`.
/// Recurses into groups; a range placeholder with no covering fresh names
/// errors (an open range past the end is legal and contributes nothing).
pub(crate) fn expand_range_refs(
    tokens: TokenStream, impl_names: &[TokenStream],
) -> Result<TokenStream, TokenStream> {
    // The impl's fresh names in document order: grouped fresh names
    // (`_Param_{g}_{i}_BatchGen_`) sorted by (group, position) — exactly the
    // order `@N` uses (the sweeper renumbers to `_Param_0..N_BatchGen_`
    // afterwards). Anything else is a user-written param and does not
    // participate in `@N` indexing.
    let mut fresh_sorted: Vec<(usize, usize, &TokenStream)> = impl_names
        .iter()
        .filter_map(|n| {
            let (g, i) = parse_grouped_fresh(&n.to_string())?;
            Some((g, i, n))
        })
        .collect();
    fresh_sorted.sort_by_key(|&(g, i, _)| (g, i));
    let names: Vec<&TokenStream> = fresh_sorted.iter().map(|&(_, _, n)| n).collect();

    let v = tokens.into_iter().collect::<Vec<_>>();
    let out = expand_at(&v, &names, 0)?;
    Ok(out.into_iter().collect())
}

fn expand_at(
    tokens: &[TokenTree], names: &[&TokenStream], depth: usize,
) -> Result<Vec<TokenTree>, TokenStream> {
    if depth > crate::util::MAX_NEST_DEPTH {
        return Err(crate::util::depth_err(tokens, ""));
    }
    let mut out = vec![];
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            TokenTree::Ident(id) => {
                let s = id.to_string();
                if let Some(range) = parse_range_fresh(&s) {
                    let count = range_len(range, names.len())?;
                    let mut first = true;
                    for k in 0..count {
                        if !first {
                            out.push(TokenTree::Punct(proc_macro2::Punct::new(',', proc_macro2::Spacing::Alone)));
                        }
                        first = false;
                        out.extend(names[range.start + k].clone());
                    }
                    i += 1;
                    continue;
                }
                out.push(tokens[i].clone());
                i += 1;
            }
            TokenTree::Group(g) => {
                if depth + 1 > crate::util::MAX_NEST_DEPTH {
                    return Err(crate::util::depth_err(&tokens[i..i + 1], ""));
                }
                let inner = g.stream().into_iter().collect::<Vec<_>>();
                let mut ng = Group::new(g.delimiter(), expand_at(&inner, names, depth + 1)?.into_iter().collect());
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
    Ok(out)
}

/// Number of fresh names a range covers; a closed range out of bounds errors,
/// an open range past the end contributes zero (legal — an arity-1 impl has
/// no "from the second element" names).
fn range_len(range: FreshRange, fresh_len: usize) -> Result<usize, TokenStream> {
    match range.end {
        Some(end) => {
            if end >= fresh_len || range.start > end {
                return Err(compile_error_str(
                    &format!(
                        "batch-impl: `@{}..={}` out of range — this impl has {} fresh \
                         generics (numbered from 0 in document order)",
                        range.start, end, fresh_len,
                    ),
                    proc_macro2::Span::call_site(),
                ));
            }
            let count = end - range.start + 1;
            if count > MAX_EXPAND {
                return Err(compile_error_str(
                    &format!(
                        "batch-impl: `@{}..={}` expands to {} elements (max {})",
                        range.start, end, count, MAX_EXPAND,
                    ),
                    proc_macro2::Span::call_site(),
                ));
            }
            Ok(count)
        }
        None => Ok(fresh_len.saturating_sub(range.start)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    fn names() -> Vec<TokenStream> {
        // Grouped fresh names (the pre-sweep form): `@N` indexes them by
        // (group, position) document order.
        vec![quote!(_Param_0_0_BatchGen_), quote!(_Param_1_0_BatchGen_), quote!(_Param_1_1_BatchGen_)]
    }

    #[test]
    fn open_range_in_generic_args() {
        let ts: TokenStream = "Wrapper < _Param_0_With_BatchGen_ >".parse().unwrap();
        let out = expand_range_refs(ts, &names()).unwrap();
        assert_eq!(
            out.to_string(),
            "Wrapper < _Param_0_0_BatchGen_ , _Param_1_0_BatchGen_ , _Param_1_1_BatchGen_ >"
        );
    }

    #[test]
    fn closed_range() {
        let ts: TokenStream = "Wrapper < _Param_1_With_2_BatchGen_ >".parse().unwrap();
        let out = expand_range_refs(ts, &names()).unwrap();
        assert_eq!(out.to_string(), "Wrapper < _Param_1_0_BatchGen_ , _Param_1_1_BatchGen_ >");
    }

    #[test]
    fn open_range_with_offset() {
        let ts: TokenStream = "Wrapper < _Param_1_With_BatchGen_ >".parse().unwrap();
        let out = expand_range_refs(ts, &names()).unwrap();
        assert_eq!(out.to_string(), "Wrapper < _Param_1_0_BatchGen_ , _Param_1_1_BatchGen_ >");
    }

    #[test]
    fn tuple_range() {
        let ts: TokenStream = "( _Param_0_With_BatchGen_ , u8 )".parse().unwrap();
        let out = expand_range_refs(ts, &names()).unwrap();
        assert_eq!(
            out.to_string(),
            "(_Param_0_0_BatchGen_ , _Param_1_0_BatchGen_ , _Param_1_1_BatchGen_ , u8)"
        );
    }

    #[test]
    fn closed_range_out_of_bounds_errors() {
        let ts: TokenStream = "Wrapper < _Param_1_With_5_BatchGen_ >".parse().unwrap();
        assert!(expand_range_refs(ts, &names()).is_err());
    }

    #[test]
    fn plain_fresh_names_untouched() {
        let ts: TokenStream = "Wrapper < _Param_0_BatchGen_ >".parse().unwrap();
        let out = expand_range_refs(ts, &names()).unwrap();
        assert_eq!(out.to_string(), "Wrapper < _Param_0_BatchGen_ >");
    }
}
