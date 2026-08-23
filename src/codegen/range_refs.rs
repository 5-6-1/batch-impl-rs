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
//! produced by `parse::resolve_at_refs` — plus the **impl-generic declaration
//! position** (`<@0..>` declares every fresh as an impl param).

use proc_macro2::{Group, TokenStream, TokenTree};

use crate::ast::fresh::{FreshRange, parse_range_fresh};
use crate::ast::{MAX_EXPAND, parse_grouped_fresh};
use crate::parse::split_at_depth0;
use crate::util::compile_error_str;

/// The impl's fresh names in document order: grouped fresh names
/// (`_Param_{g}_{i}_BatchGen_`) sorted by (group, position) — exactly the
/// order `@N` uses (the sweeper renumbers to `_Param_0..N_BatchGen_`
/// afterwards). Anything else is a user-written param and does not
/// participate in `@N` indexing. Each entry carries its (group, position)
/// so a grouped range (`@L_N..`) can slice within its group.
fn sorted_fresh(impl_names: &[TokenStream]) -> Vec<(usize, usize, &TokenStream)> {
    let mut fresh_sorted: Vec<(usize, usize, &TokenStream)> = impl_names
        .iter()
        .filter_map(|n| {
            let (g, i) = parse_grouped_fresh(&n.to_string())?;
            Some((g, i, n))
        })
        .collect();
    fresh_sorted.sort_by_key(|&(g, i, _)| (g, i));
    fresh_sorted
}

/// The entries of one generator group (sorted by position within the group);
/// an unknown group errors (mirrors `@g_i`'s `at_group_out_of_range`).
/// The nested `&TokenStream` reference needs an explicit lifetime; clippy's
/// `needless_lifetimes` does not account for elision across the inner ref.
#[allow(clippy::needless_lifetimes)]
pub(crate) fn group_fresh<'a>(
    fresh: &'a [(usize, usize, &'a TokenStream)], group: usize, span: proc_macro2::Span,
) -> Result<&'a [(usize, usize, &'a TokenStream)], TokenStream> {
    let start = fresh.iter().position(|&(g, _, _)| g == group).ok_or_else(|| {
        compile_error_str(
            &format!(
                "batch-impl: `@{}_..` group {} does not exist — this impl has \
                 no generator group {}",
                group, group, group,
            ),
            span,
        )
    })?;
    let end =
        fresh[start..].iter().position(|&(g, _, _)| g != group).map_or(fresh.len(), |p| start + p);
    Ok(&fresh[start..end])
}

/// Number of entries a range covers within its slice scope; a closed range
/// out of bounds errors, an open range past the end contributes zero.
pub(crate) fn range_count(
    range: FreshRange, scope_len: usize, span: proc_macro2::Span,
) -> Result<usize, TokenStream> {
    match range.end {
        Some(end) => {
            if end >= scope_len || range.start > end {
                return Err(compile_error_str(
                    &format!(
                        "batch-impl: `@{}..={}` out of range — this scope has {} fresh \
                         generics (numbered from 0 in document order)",
                        range.start, end, scope_len,
                    ),
                    span,
                ));
            }
            let count = end - range.start + 1;
            if count > MAX_EXPAND {
                return Err(compile_error_str(
                    &format!(
                        "batch-impl: `@{}..={}` expands to {} elements (max {})",
                        range.start, end, count, MAX_EXPAND,
                    ),
                    span,
                ));
            }
            Ok(count)
        }
        None => Ok(scope_len.saturating_sub(range.start)),
    }
}

/// Resolves a range against the sorted fresh list: a flat `@N..` slices by
/// flattened index, a grouped `@L_N..` slices within group L. Returns the
/// covered entries — empty for an open range past the end, an error for a
/// closed range out of bounds or an unknown group.
fn range_entries<'a>(
    range: FreshRange, fresh: &'a [(usize, usize, &'a TokenStream)],
) -> Result<Vec<&'a TokenStream>, TokenStream> {
    let slice: &[(usize, usize, &TokenStream)] = match range.group {
        Some(l) => group_fresh(fresh, l, proc_macro2::Span::call_site())?,
        None => fresh,
    };
    let count = range_count(range, slice.len(), proc_macro2::Span::call_site())?;
    // An open range past the end contributes nothing (`count` is 0) — return
    // early, never slice `slice[range.start..]` (the index can exceed a
    // zero-length scope and panic).
    if count == 0 {
        return Ok(vec![]);
    }
    Ok(slice[range.start..range.start + count].iter().map(|&(_, _, n)| n).collect())
}

/// Expands every `@N..` / `@N..M` range placeholder in `tokens` against the
/// impl's fresh names: `_Param_0_With_BatchGen_` → `P0, P1, P2, ...` (each
/// fresh name an element), `_Param_1_With_3_BatchGen_` → `P1, P2, P3`, and
/// the grouped forms `_Param_0_1_With_BatchGen_` slice within group 0.
/// Recurses into groups; a range placeholder with no covering fresh names
/// errors (an open range past the end is legal and contributes nothing).
pub(crate) fn expand_range_refs(
    tokens: TokenStream, impl_names: &[TokenStream],
) -> Result<TokenStream, TokenStream> {
    let fresh = sorted_fresh(impl_names);

    let v = tokens.into_iter().collect::<Vec<_>>();
    let out = expand_at(&v, &fresh, 0)?;
    Ok(out.into_iter().collect())
}

/// Expands a range placeholder in the **impl-generic declaration position**:
/// `<@0..>` declares every fresh the range covers as an impl generic param.
/// The declaration list is rebuilt in place — a placeholder entry
/// (`_Param_N_With[_M]_BatchGen_`, with its bound) becomes one bare entry per
/// fresh name. Runs before `merge_dup_params`, so a range declaration that
/// overlaps a generator's fresh declarations collapses cleanly.
pub(crate) fn expand_range_decls(
    impl_generics: &mut Vec<(TokenStream, Option<crate::ast::Ty>)>, impl_names: &[TokenStream],
) -> Result<(), TokenStream> {
    let fresh = sorted_fresh(impl_names);
    let mut out: Vec<(TokenStream, Option<crate::ast::Ty>)> = vec![];
    for (name, bound) in impl_generics.iter() {
        let s = name.to_string();
        if let Some(range) = parse_range_fresh(&s) {
            for n in range_entries(range, &fresh)? {
                out.push(((*n).clone(), None));
            }
        } else {
            out.push((name.clone(), bound.clone()));
        }
    }
    *impl_generics = out;
    Ok(())
}

/// Drops **orphaned empty elements** from an expanded tuple and rejoins: a
/// `@N..` range that re-opened to zero entries leaves an empty element
/// (`(,)` / `(, P1,)` / `(P0, ,)`), which is not valid Rust. A trailing empty
/// chunk is a legal trailing comma (`(P0,)` / `(expr,)`) and stays untouched;
/// an ordinary tuple is returned verbatim (the split is only a scan — flat
/// `<...>` in an element must never be re-joined).
fn fold_empty_tuple(tokens: &[TokenTree]) -> TokenStream {
    let chunks: Vec<&[TokenTree]> = split_at_depth0(tokens, ',').into_iter().collect();
    // The trailing chunk may be empty (a legal trailing comma) — exclude it
    // from the orphan check, not from the output.
    let trimmed: Vec<&[TokenTree]> = if chunks.last().is_some_and(|c| c.is_empty()) {
        chunks[..chunks.len() - 1].to_vec()
    } else {
        chunks
    };
    // No orphaned empties: the tuple is ordinary (a lone trailing comma is
    // fine) — return verbatim, never re-join (the split cannot see through
    // flat `<...>`, so a re-join would corrupt elements containing commas).
    if !trimmed.iter().any(|c| c.is_empty()) {
        return tokens.iter().cloned().collect();
    }
    // Orphaned empties: drop them and rejoin; a single surviving element
    // keeps its trailing comma (`(A)` is a group, not a 1-tuple).
    let elems: Vec<&[TokenTree]> = trimmed.into_iter().filter(|c| !c.is_empty()).collect();
    let mut out = TokenStream::new();
    for (i, e) in elems.iter().enumerate() {
        if i > 0 {
            out.extend(std::iter::once(TokenTree::Punct(proc_macro2::Punct::new(
                ',',
                proc_macro2::Spacing::Alone,
            ))));
        }
        out.extend(e.iter().cloned());
    }
    if elems.len() == 1 {
        out.extend(std::iter::once(TokenTree::Punct(proc_macro2::Punct::new(
            ',',
            proc_macro2::Spacing::Alone,
        ))));
    }
    out
}

fn expand_at(
    tokens: &[TokenTree], fresh: &[(usize, usize, &TokenStream)], depth: usize,
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
                    let mut first = true;
                    for n in range_entries(range, fresh)? {
                        if !first {
                            out.push(TokenTree::Punct(proc_macro2::Punct::new(
                                ',',
                                proc_macro2::Spacing::Alone,
                            )));
                        }
                        first = false;
                        out.extend(n.clone());
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
                // Empty-tuple folding applies **only to a tuple whose top
                // level holds a range placeholder** (`(@0..,)` — the folded
                // `_Param_N_With[_M]_BatchGen_` ident): re-opening the range
                // is the sole source of orphaned commas. An ordinary paren
                // (`(expr,)`, `(A)`) never reaches the fold, so its commas —
                // including flat `<...>` inside an element — are never
                // re-joined.
                let has_range_placeholder = inner.iter().any(|t| {
                    matches!(t, TokenTree::Ident(id)
                        if crate::ast::fresh::parse_range_fresh(&id.to_string()).is_some())
                });
                let foldable =
                    g.delimiter() == proc_macro2::Delimiter::Parenthesis && has_range_placeholder;
                let expanded: Vec<TokenTree> =
                    expand_at(&inner, fresh, depth + 1)?.into_iter().collect();
                // Empty-tuple folding: a range that re-opened to zero entries
                // leaves orphaned empties (`(,)` / `(, P1,)` / `(P0, ,)`) —
                // drop them and rejoin, restoring a valid tuple (`()` /
                // `(P1,)` / `(P0,)`). Only foldable tuples (those that held
                // a placeholder) reach this branch.
                let inner_ts = if foldable {
                    fold_empty_tuple(&expanded)
                } else {
                    expanded.into_iter().collect()
                };
                let mut ng = Group::new(g.delimiter(), inner_ts);
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

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    fn names() -> Vec<TokenStream> {
        // Grouped fresh names (the pre-sweep form): `@N` indexes them by
        // (group, position) document order.
        vec![
            quote!(_Param_0_0_BatchGen_),
            quote!(_Param_1_0_BatchGen_),
            quote!(_Param_1_1_BatchGen_),
        ]
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

    #[test]
    fn decl_position_open_range() {
        // `<@0..>` — a range placeholder as an impl-generic declaration
        // expands into one bare declaration per fresh.
        let mut gens: Vec<(TokenStream, Option<crate::ast::Ty>)> =
            vec![("_Param_0_With_BatchGen_".parse().unwrap(), None)];
        expand_range_decls(&mut gens, &names()).unwrap();
        let got: Vec<String> = gens.iter().map(|(n, _)| n.to_string()).collect();
        assert_eq!(got, ["_Param_0_0_BatchGen_", "_Param_1_0_BatchGen_", "_Param_1_1_BatchGen_"]);
    }

    #[test]
    fn decl_position_closed_range() {
        let mut gens: Vec<(TokenStream, Option<crate::ast::Ty>)> =
            vec![("_Param_1_With_2_BatchGen_".parse().unwrap(), None)];
        expand_range_decls(&mut gens, &names()).unwrap();
        let got: Vec<String> = gens.iter().map(|(n, _)| n.to_string()).collect();
        assert_eq!(got, ["_Param_1_0_BatchGen_", "_Param_1_1_BatchGen_"]);
    }

    #[test]
    fn decl_position_mixed_with_plain() {
        // A user param and a range declaration coexist; the plain one stays.
        let mut gens: Vec<(TokenStream, Option<crate::ast::Ty>)> =
            vec![("X".parse().unwrap(), None), ("_Param_0_With_BatchGen_".parse().unwrap(), None)];
        expand_range_decls(&mut gens, &names()).unwrap();
        let got: Vec<String> = gens.iter().map(|(n, _)| n.to_string()).collect();
        assert_eq!(
            got,
            ["X", "_Param_0_0_BatchGen_", "_Param_1_0_BatchGen_", "_Param_1_1_BatchGen_"]
        );
    }

    #[test]
    fn decl_position_closed_out_of_bounds_errors() {
        let mut gens: Vec<(TokenStream, Option<crate::ast::Ty>)> =
            vec![("_Param_0_With_5_BatchGen_".parse().unwrap(), None)];
        assert!(expand_range_decls(&mut gens, &names()).is_err());
    }

    #[test]
    fn grouped_range_open_in_generic_args() {
        // `@0_0..` — group 0 from position 0: its only entry.
        let ts: TokenStream = "Wrapper < _Param_0_0_With_BatchGen_ >".parse().unwrap();
        let out = expand_range_refs(ts, &names()).unwrap();
        assert_eq!(out.to_string(), "Wrapper < _Param_0_0_BatchGen_ >");
    }

    #[test]
    fn grouped_range_open_group1() {
        // `@1_0..` — group 1 from position 0: both entries of group 1.
        let ts: TokenStream = "Wrapper < _Param_1_0_With_BatchGen_ >".parse().unwrap();
        let out = expand_range_refs(ts, &names()).unwrap();
        assert_eq!(out.to_string(), "Wrapper < _Param_1_0_BatchGen_ , _Param_1_1_BatchGen_ >");
    }

    #[test]
    fn grouped_range_closed_in_generic_args() {
        // `@1_0..=0` — group 1, positions 0..=0 → just the first.
        let ts: TokenStream = "Wrapper < _Param_1_0_With_0_BatchGen_ >".parse().unwrap();
        let out = expand_range_refs(ts, &names()).unwrap();
        assert_eq!(out.to_string(), "Wrapper < _Param_1_0_BatchGen_ >");
    }

    #[test]
    fn grouped_range_second_group_tail() {
        // `@1_1..` — group 1 from position 1: only the group's tail.
        let ts: TokenStream = "Wrapper < _Param_1_1_With_BatchGen_ >".parse().unwrap();
        let out = expand_range_refs(ts, &names()).unwrap();
        assert_eq!(out.to_string(), "Wrapper < _Param_1_1_BatchGen_ >");
    }

    #[test]
    fn grouped_range_unknown_group_errors() {
        let ts: TokenStream = "Wrapper < _Param_3_0_With_BatchGen_ >".parse().unwrap();
        assert!(expand_range_refs(ts, &names()).is_err());
    }

    #[test]
    fn grouped_range_out_of_group_errors() {
        // Group 0 has 1 entry; `@0_2..=3` is out of range.
        let ts: TokenStream = "Wrapper < _Param_0_2_With_3_BatchGen_ >".parse().unwrap();
        assert!(expand_range_refs(ts, &names()).is_err());
    }

    #[test]
    fn ordinary_tuple_trailing_comma_preserved() {
        // a plain tuple element containing flat `<...>` (its commas must not
        // be re-joined) keeps its trailing comma — `(expr,)` stays a 1-tuple
        let ts: TokenStream = quote::quote!(
            fn f() -> (f64,) {
                (<f64 as Module<f64, f64>>::scale(&r, self.0),)
            }
        );
        let out = expand_range_refs(ts, &names()).unwrap();
        assert!(out.to_string().contains("self . 0) ,) }"), "{out}");
    }

    #[test]
    fn orphan_empties_still_fold() {
        // the range re-opened to zero entries: `(P0, ,)` folds to `(P0,)`
        let ts: TokenStream = quote::quote!(_Param_0_0_BatchGen_ , ,);
        let folded = fold_empty_tuple(&ts.into_iter().collect::<Vec<_>>());
        assert_eq!(folded.to_string(), "_Param_0_0_BatchGen_ ,");
    }
}
