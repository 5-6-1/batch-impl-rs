//! `@N..` / `@N..M` range-reference expansion at the token level: a fresh
//! reference in its self-delimiting carrier form (`@` + Brace group holding
//! the spelled reference — see [`crate::ast::fresh::FreshRef`]) in a rendered
//! type expands into the impl's fresh names — one position becomes several
//! (`Wrapper<@{0..}>` → `Wrapper<P0, P1, P2>`). The parse layer carries the
//! reference structurally (`TyKind::Fresh`) and renders it to the carrier;
//! only here does it re-open into the real generic list.
//!
//! Where-predicate ranges are handled by `resolve_where_at` on the raw flat
//! spelling (predicate-subject expansion); this file covers the **type
//! positions** (target type, trait args, impl-generic bounds) — plus the
//! **impl-generic declaration position** (`<@0..>` declares every fresh as an
//! impl param).

use proc_macro2::{Group, Span, TokenStream, TokenTree};

use crate::ast::fresh::{FreshEnd, FreshRef};
use crate::ast::MAX_EXPAND;
use crate::codegen::FreshCtx;
use crate::parse::split_at_depth0;
use crate::util::compile_error_str;

/// Number of entries a range covers within its slice scope; a closed range
/// out of bounds errors, an open range past the end contributes zero.
pub(crate) fn range_count(
    r: &FreshRef, scope_len: usize, span: Span,
) -> Result<usize, TokenStream> {
    match r.end {
        FreshEnd::Closed(end) => {
            if end >= scope_len || r.start > end {
                return Err(compile_error_str(
                    &format!(
                        "batch-impl: `@{}..={}` out of range — this scope has {} fresh \
                         generics (numbered from 0 in document order)",
                        r.start, end, scope_len,
                    ),
                    span,
                ));
            }
            let count = end - r.start + 1;
            if count > MAX_EXPAND {
                return Err(compile_error_str(
                    &format!(
                        "batch-impl: `@{}..={}` expands to {} elements (max {})",
                        r.start, end, count, MAX_EXPAND,
                    ),
                    span,
                ));
            }
            Ok(count)
        }
        _ => Ok(scope_len.saturating_sub(r.start)),
    }
}

/// Resolves a reference against the sorted fresh list: a flat ref slices by
/// flattened index, a grouped one slices within its group. Returns the covered
/// entries — empty for an open range past the end, an error for a closed
/// range out of bounds or an unknown group. A single-position ref yields its
/// own name.
fn range_entries<'a>(
    r: &FreshRef, ctx: &'a FreshCtx<'a>,
) -> Result<Vec<&'a TokenStream>, TokenStream> {
    let slice: &[(usize, usize, &TokenStream)] = match r.group {
        Some(l) => ctx.group(l, Span::call_site())?,
        None => &ctx.names,
    };
    if let FreshEnd::Single = r.end {
        // A single position indexes the (possibly grouped) list directly.
        let Some(&(_, _, n)) = slice.get(r.start) else {
            return Err(crate::codegen::at_num_out_of_range(
                r.start,
                slice.len(),
                Span::call_site(),
            ));
        };
        return Ok(vec![n]);
    }
    let count = range_count(r, slice.len(), Span::call_site())?;
    // An open range past the end contributes nothing (`count` is 0) — return
    // early, never slice `slice[r.start..]` (the index can exceed a
    // zero-length scope and panic).
    if count == 0 {
        return Ok(vec![]);
    }
    Ok(slice[r.start..r.start + count].iter().map(|&(_, _, n)| n).collect())
}

/// Expands every fresh reference (carrier form `@{...}`) in `tokens` against
/// the impl's fresh names: `@{0..}` → `P0, P1, P2, ...` (each fresh name an
/// element), `@{1..=3}` → `P1, P2, P3`, and the grouped forms `@{0_1..}`
/// slice within group 0. Recurses into groups; a reference with no covering
/// fresh names errors (an open range past the end is legal and contributes
/// nothing).
pub(crate) fn expand_range_refs(
    tokens: TokenStream, ctx: &FreshCtx,
) -> Result<TokenStream, TokenStream> {
    let v = tokens.into_iter().collect::<Vec<_>>();
    let out = expand_at(&v, ctx, 0)?;
    Ok(out.into_iter().collect())
}

/// Expands a fresh reference in the **impl-generic declaration position**:
/// `<@0..>` declares every fresh the range covers as an impl generic param.
/// The declaration list is rebuilt in place — a reference entry (with its
/// bound) becomes one bare entry per covered fresh name. Runs before
/// `merge_dup_params`, so a range declaration that overlaps a generator's
/// fresh declarations collapses cleanly.
pub(crate) fn expand_range_decls(
    impl_generics: &mut Vec<(TokenStream, Option<crate::ast::Ty>)>, ctx: &FreshCtx,
) -> Result<(), TokenStream> {
    let mut out: Vec<(TokenStream, Option<crate::ast::Ty>)> = vec![];
    for (name, bound) in impl_generics.iter() {
        match decl_fresh_ref(name) {
            Some(r) => {
                for n in range_entries(&r, ctx)? {
                    out.push(((*n).clone(), None));
                }
            }
            None => out.push((name.clone(), bound.clone())),
        }
    }
    *impl_generics = out;
    Ok(())
}

/// Recognizes a declaration entry that is a bare fresh-ref carrier:
/// `@` followed by a Brace group (and nothing else).
fn decl_fresh_ref(name: &TokenStream) -> Option<FreshRef> {
    let v: Vec<_> = name.clone().into_iter().collect();
    match v.as_slice() {
        [TokenTree::Punct(p), TokenTree::Group(g)]
            if p.as_char() == '@' && g.delimiter() == proc_macro2::Delimiter::Brace =>
        {
            let inner: String =
                g.stream().into_iter().map(|t| t.to_string()).collect::<Vec<_>>().join("");
            FreshRef::parse(&inner)
        }
        _ => None,
    }
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
    tokens: &[TokenTree], ctx: &FreshCtx, depth: usize,
) -> Result<Vec<TokenTree>, TokenStream> {
    if depth > crate::util::MAX_NEST_DEPTH {
        return Err(crate::util::depth_err(tokens, ""));
    }
    let mut out = vec![];
    let mut i = 0;
    while i < tokens.len() {
        // A fresh-reference carrier: `@` + Brace group. A single position
        // splices one name; a range splices the covered names comma-separated.
        if let TokenTree::Punct(p) = &tokens[i]
            && p.as_char() == '@'
            && matches!(tokens.get(i + 1), Some(TokenTree::Group(g))
                if g.delimiter() == proc_macro2::Delimiter::Brace)
        {
            let inner: String = match tokens.get(i + 1) {
                Some(TokenTree::Group(g)) => g
                    .stream()
                    .into_iter()
                    .map(|t| t.to_string())
                    .collect::<Vec<_>>()
                    .join(""),
                _ => unreachable!("matched above"),
            };
            let r = FreshRef::parse(&inner).ok_or_else(|| {
                compile_error_str(
                    "batch-impl: `@{...}` must hold a position reference \
                     (e.g. `@{0}`, `@{1_0..}`, `@{0..=3}`)",
                    p.span(),
                )
            })?;
            let names = range_entries(&r, ctx)?;
            let separated = r.is_range();
            for (k, &n) in names.iter().enumerate() {
                if separated && k > 0 {
                    out.push(TokenTree::Punct(proc_macro2::Punct::new(
                        ',',
                        proc_macro2::Spacing::Alone,
                    )));
                }
                out.extend(n.clone());
            }
            i += 2;
            continue;
        }
        match &tokens[i] {
            TokenTree::Group(g) => {
                if depth + 1 > crate::util::MAX_NEST_DEPTH {
                    return Err(crate::util::depth_err(&tokens[i..i + 1], ""));
                }
                let inner = g.stream().into_iter().collect::<Vec<_>>();
                // Empty-tuple folding applies **only to a tuple whose top
                // level holds a range reference** (`(@{0..},)`): re-opening
                // the range is the sole source of orphaned commas. An
                // ordinary paren (`(expr,)`, `(A)`) never reaches the fold,
                // so its commas — including flat `<...>` inside an element —
                // are never re-joined.
                let has_range_ref =
                    inner.windows(2).any(|w| is_fresh_carrier_pair(&w[0], &w[1]));
                let foldable =
                    g.delimiter() == proc_macro2::Delimiter::Parenthesis && has_range_ref;
                let expanded: Vec<TokenTree> =
                    expand_at(&inner, ctx, depth + 1)?.into_iter().collect();
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

/// Whether a token pair is a fresh-ref carrier: a `@` punct directly
/// followed by a Brace group.
fn is_fresh_carrier_pair(at: &TokenTree, g: &TokenTree) -> bool {
    matches!(at, TokenTree::Punct(p) if p.as_char() == '@')
        && matches!(g, TokenTree::Group(g) if g.delimiter() == proc_macro2::Delimiter::Brace)
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    fn fresh_ctx() -> FreshCtx<'static> {
        // Leak is fine in tests: the ctx borrows the names for the call only.
        FreshCtx::new(Box::leak(Box::new(names())))
    }
    fn names() -> Vec<TokenStream> {
        // Grouped fresh names (the pre-sweep form): `@N` indexes them by
        // (group, position) document order.
        vec![
            quote!(_Param_0_0_BatchGen_),
            quote!(_Param_1_0_BatchGen_),
            quote!(_Param_1_1_BatchGen_),
        ]
    }
    fn carrier(spell: &str) -> TokenStream {
        let r = FreshRef::parse(spell).unwrap();
        crate::ast::fresh::fresh_ref_tokens(r, proc_macro2::Span::call_site())
    }
    /// Builds `prefix @carrier suffix` — a rendered type holding one reference.
    fn wrap(prefix: &str, spell: &str, suffix: &str) -> TokenStream {
        let mut ts: TokenStream = prefix.parse().unwrap();
        ts.extend(carrier(spell));
        ts.extend(suffix.parse::<TokenStream>().unwrap());
        ts
    }

    #[test]
    fn open_range_in_generic_args() {
        let out = expand_range_refs(wrap("Wrapper <", "0..", ">"), &fresh_ctx()).unwrap();
        assert_eq!(
            out.to_string(),
            "Wrapper < _Param_0_0_BatchGen_ , _Param_1_0_BatchGen_ , _Param_1_1_BatchGen_ >"
        );
    }

    #[test]
    fn closed_range() {
        let out = expand_range_refs(wrap("Wrapper <", "1..=2", ">"), &fresh_ctx()).unwrap();
        assert_eq!(out.to_string(), "Wrapper < _Param_1_0_BatchGen_ , _Param_1_1_BatchGen_ >");
    }

    #[test]
    fn open_range_with_offset() {
        let out = expand_range_refs(wrap("Wrapper <", "1..", ">"), &fresh_ctx()).unwrap();
        assert_eq!(out.to_string(), "Wrapper < _Param_1_0_BatchGen_ , _Param_1_1_BatchGen_ >");
    }

    #[test]
    fn tuple_range() {
        // `@{...}` is literally writable Rust punctuation + brace group.
        let ts: TokenStream = "(@{0..} , u8)".parse().unwrap();
        let out = expand_range_refs(ts, &fresh_ctx()).unwrap();
        assert_eq!(
            out.to_string(),
            "(_Param_0_0_BatchGen_ , _Param_1_0_BatchGen_ , _Param_1_1_BatchGen_ , u8)"
        );
    }

    #[test]
    fn closed_range_out_of_bounds_errors() {
        assert!(expand_range_refs(wrap("Wrapper <", "1..=5", ">"), &fresh_ctx()).is_err());
    }

    #[test]
    fn plain_fresh_names_untouched() {
        let ts: TokenStream = "Wrapper < _Param_0_BatchGen_ >".parse().unwrap();
        let out = expand_range_refs(ts, &fresh_ctx()).unwrap();
        assert_eq!(out.to_string(), "Wrapper < _Param_0_BatchGen_ >");
    }

    #[test]
    fn single_position_resolves_one_name() {
        let out = expand_range_refs(wrap("Wrapper <", "2", ">"), &fresh_ctx()).unwrap();
        assert_eq!(out.to_string(), "Wrapper < _Param_1_1_BatchGen_ >");
    }

    #[test]
    fn decl_position_open_range() {
        // `<@{0..}>` — a range reference as an impl-generic declaration
        // expands into one bare declaration per fresh.
        let mut gens: Vec<(TokenStream, Option<crate::ast::Ty>)> = vec![(carrier("0.."), None)];
        expand_range_decls(&mut gens, &fresh_ctx()).unwrap();
        let got: Vec<String> = gens.iter().map(|(n, _)| n.to_string()).collect();
        assert_eq!(got, ["_Param_0_0_BatchGen_", "_Param_1_0_BatchGen_", "_Param_1_1_BatchGen_"]);
    }

    #[test]
    fn decl_position_closed_range() {
        let mut gens: Vec<(TokenStream, Option<crate::ast::Ty>)> = vec![(carrier("1..=2"), None)];
        expand_range_decls(&mut gens, &fresh_ctx()).unwrap();
        let got: Vec<String> = gens.iter().map(|(n, _)| n.to_string()).collect();
        assert_eq!(got, ["_Param_1_0_BatchGen_", "_Param_1_1_BatchGen_"]);
    }

    #[test]
    fn decl_position_mixed_with_plain() {
        // A user param and a range declaration coexist; the plain one stays.
        let mut gens: Vec<(TokenStream, Option<crate::ast::Ty>)> =
            vec![("X".parse().unwrap(), None), (carrier("0.."), None)];
        expand_range_decls(&mut gens, &fresh_ctx()).unwrap();
        let got: Vec<String> = gens.iter().map(|(n, _)| n.to_string()).collect();
        assert_eq!(
            got,
            ["X", "_Param_0_0_BatchGen_", "_Param_1_0_BatchGen_", "_Param_1_1_BatchGen_"]
        );
    }

    #[test]
    fn decl_position_closed_out_of_bounds_errors() {
        let mut gens: Vec<(TokenStream, Option<crate::ast::Ty>)> = vec![(carrier("0..=5"), None)];
        assert!(expand_range_decls(&mut gens, &fresh_ctx()).is_err());
    }

    #[test]
    fn grouped_range_open_in_generic_args() {
        // `@{0_0..}` — group 0 from position 0: its only entry.
        let out = expand_range_refs(wrap("Wrapper <", "0_0..", ">"), &fresh_ctx()).unwrap();
        assert_eq!(out.to_string(), "Wrapper < _Param_0_0_BatchGen_ >");
    }

    #[test]
    fn grouped_range_open_group1() {
        // `@{1_0..}` — group 1 from position 0: both entries of group 1.
        let out = expand_range_refs(wrap("Wrapper <", "1_0..", ">"), &fresh_ctx()).unwrap();
        assert_eq!(out.to_string(), "Wrapper < _Param_1_0_BatchGen_ , _Param_1_1_BatchGen_ >");
    }

    #[test]
    fn grouped_range_closed_in_generic_args() {
        // `@{1_0..=0}` — group 1, positions 0..=0 → just the first.
        let out = expand_range_refs(wrap("Wrapper <", "1_0..=0", ">"), &fresh_ctx()).unwrap();
        assert_eq!(out.to_string(), "Wrapper < _Param_1_0_BatchGen_ >");
    }

    #[test]
    fn grouped_range_second_group_tail() {
        // `@{1_1..}` — group 1 from position 1: only the group's tail.
        let out = expand_range_refs(wrap("Wrapper <", "1_1..", ">"), &fresh_ctx()).unwrap();
        assert_eq!(out.to_string(), "Wrapper < _Param_1_1_BatchGen_ >");
    }

    #[test]
    fn grouped_range_unknown_group_errors() {
        assert!(expand_range_refs(wrap("Wrapper <", "3_0..", ">"), &fresh_ctx()).is_err());
    }

    #[test]
    fn grouped_range_out_of_group_errors() {
        // Group 0 has 1 entry; `@{0_2..=3}` is out of range.
        assert!(expand_range_refs(wrap("Wrapper <", "0_2..=3", ">"), &fresh_ctx()).is_err());
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
        let out = expand_range_refs(ts, &fresh_ctx()).unwrap();
        assert!(out.to_string().contains("self . 0) ,) }"), "{out}");
    }
}