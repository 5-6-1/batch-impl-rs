//! `@` position references in where predicates: `@N` / `@g_i` / `@all_fresh`
//! / `@N..M` resolve against the impl's fresh generics (document order for
//! `@N`, exact generating site for `@g_i`, batch forms for the rest).

use proc_macro2::{Group, Punct, Spacing, TokenStream, TokenTree};

use super::{at_group_out_of_range, at_num_out_of_range};
use crate::ast::{MAX_EXPAND, parse_grouped_fresh};
use crate::util::{compile_err, compile_error_str};

/// Macro-meta position references in where predicates: `@N` → the N-th fresh
/// generic in document order (grouped fresh names `_Param_{g}_{i}_BatchGen_`
/// sorted by (group, position), which is exactly the order the codegen
/// sweeper renumbers to `_Param_0..N_BatchGen_`) — user-written params are
/// addressed by their own names; `@N` exists exactly because fresh names are
/// unknowable. `@N` out of range or a non-position digit / other token after
/// `@` errors. `@trait` is resolved earlier (constant stage for batch_impl,
/// segment-level replacement for batch_trait!) and never reaches here.
/// Blanket-wrapped where is pre-resolved; only user where predicates are
/// handled here.
/// Resolves every where predicate of an impl: rejects a bare splat subject
/// and expands the `@` position references (`@N` / `@g_i` / `@all_fresh` /
/// `@N..M`) against `impl_name_streams`. All errors are collected and
/// returned at once (the caller emits only the errors — no partial impl).
pub(crate) fn resolve_where_predicates(
    where_clauses: &[TokenStream], impl_name_streams: &[TokenStream],
) -> Result<Vec<TokenStream>, Vec<TokenStream>> {
    let mut where_resolved = vec![];
    let mut errs = vec![];
    for pred in where_clauses {
        // A bare splat as a predicate subject has no defined semantics
        // (`*(A,B): Trait` would expand to `A, B: Trait` — a predicate is a
        // constraint, not a parameter list). Reject with a clear message;
        // splats inside a predicate (`X: Trait<*(A,B)>`) and tuple
        // predicates (`(*(A,B)): Trait`) are fine — they expand legally.
        let head = pred.clone().into_iter().collect::<Vec<_>>();
        if matches!(head.as_slice(),
            [TokenTree::Punct(p), TokenTree::Group(g), ..]
            if p.as_char() == '*'
                && matches!(
                    g.delimiter(),
                    proc_macro2::Delimiter::Parenthesis
                        | proc_macro2::Delimiter::Bracket
                )
        ) {
            errs.push(compile_err!(
                "batch-impl: a bare splat cannot be a where-predicate subject \
                 (`*(A,B): Trait`); wrap it in a tuple (`(*(A,B)): Trait`) or \
                 write separate predicates"
            ));
            continue;
        }
        match resolve_where_at(pred, impl_name_streams) {
            // An empty result (a `@N..` open range with no fresh past N, or a
            // trailing-comma empty segment) contributes no predicate — skip
            // it instead of emitting a dangling comma into the where clause.
            Ok(p) if !p.is_empty() => where_resolved.push(p),
            Ok(_) => {}
            Err(e) => errs.push(e),
        }
    }
    if errs.is_empty() { Ok(where_resolved) } else { Err(errs) }
}

pub(crate) fn resolve_where_at(
    pred: &TokenStream, impl_names: &[TokenStream],
) -> Result<TokenStream, TokenStream> {
    // Fresh params sorted by (group, position) — the sweep order, so `@N`
    // matches the final `_Param_{N}_BatchGen_` the sweeper will emit. The
    // parse result rides in the tuple (filter_map), so the sort key can
    // never unwrap — the library promises no panics, even on adversarial
    // input or internal invariant drift.
    let mut fresh_sorted: Vec<(usize, usize, &TokenStream)> = impl_names
        .iter()
        .filter_map(|n| {
            let (g, i) = parse_grouped_fresh(&n.to_string())?;
            Some((g, i, n))
        })
        .collect();
    fresh_sorted.sort_by_key(|&(g, i, _)| (g, i));
    let tokens = pred.clone().into_iter().collect::<Vec<_>>();
    let mut out = vec![];
    let mut i = 0;
    while i < tokens.len() {
        if let TokenTree::Punct(p) = &tokens[i]
            && p.as_char() == '@'
        {
            match tokens.get(i + 1) {
                Some(TokenTree::Ident(id)) if id == "all_fresh" => {
                    // `@all_fresh: Bound` → every fresh generic gets the
                    // predicate tail (`_Param_0_: Bound, _Param_1_: Bound,
                    // ...`) — comma-separated, subject-only.
                    if fresh_sorted.is_empty() {
                        return Err(compile_error_str(
                            "batch-impl: `@all_fresh` in a where predicate but this impl has no fresh generics",
                            tokens[i].span(),
                        ));
                    }
                    if fresh_sorted.len() > MAX_EXPAND {
                        return Err(compile_err!(
                            "batch-impl: `@all_fresh` expands to {} predicates (max {}); use `@N..M` for a subset",
                            fresh_sorted.len(),
                            MAX_EXPAND
                        ));
                    }
                    let tail = resolve_tail(&tokens[i + 2..], impl_names)?;
                    emit_fresh_predicates(&mut out, &fresh_sorted, &tail);
                    i = tokens.len();
                    continue;
                }
                Some(TokenTree::Literal(lit)) => {
                    let s = lit.to_string();
                    // `@L_N..` grouped open range / `@L_N..M` / `@L_N..=M` —
                    // within generator group L (stable across array dispatch,
                    // like `@g_i`). Slices the group's fresh entries by
                    // in-group position.
                    if let Some((group, start)) = parse_group_start(&s)
                        && matches!(tokens.get(i + 2), Some(TokenTree::Punct(p)) if p.as_char() == '.')
                        && matches!(tokens.get(i + 3), Some(TokenTree::Punct(p)) if p.as_char() == '.')
                    {
                        let slice = crate::codegen::range_refs::group_fresh(
                            &fresh_sorted,
                            group,
                            tokens[i].span(),
                        )?;
                        let mut consumed = 4;
                        if matches!(tokens.get(i + 4), Some(TokenTree::Punct(p)) if p.as_char() == '=')
                        {
                            consumed += 1;
                        }
                        let end = match tokens.get(i + consumed) {
                            Some(TokenTree::Literal(el)) => {
                                let Some(e) = el.to_string().parse::<usize>().ok() else {
                                    return Err(compile_error_str(
                                        "batch-impl: a `@N..M` range must end with a number (e.g. `@0..=2`)",
                                        tokens[i].span(),
                                    ));
                                };
                                consumed += 1;
                                Some(e)
                            }
                            _ => None,
                        };
                        let range =
                            crate::ast::fresh::FreshRange { group: Some(group), start, end };
                        let count = crate::codegen::range_refs::range_count(
                            range,
                            slice.len(),
                            tokens[i].span(),
                        )?;
                        let tail = resolve_tail(&tokens[i + consumed..], impl_names)?;
                        emit_fresh_predicates(&mut out, &slice[start..start + count], &tail);
                        i = tokens.len();
                        continue;
                    }
                    // `@N..` open range: from N to the last fresh — empty
                    // when N is past the end (legal: an arity-1 impl
                    // contributes no "from the second element" predicate,
                    // e.g. `@1..: Module<...>`).
                    if let Ok(start) = s.parse::<usize>()
                        && matches!(tokens.get(i + 2), Some(TokenTree::Punct(p)) if p.as_char() == '.')
                        && matches!(tokens.get(i + 3), Some(TokenTree::Punct(p)) if p.as_char() == '.')
                        && !matches!(tokens.get(i + 4), Some(TokenTree::Punct(p)) if p.as_char() == '=')
                        && !matches!(tokens.get(i + 4), Some(TokenTree::Literal(_)))
                    {
                        let count = fresh_sorted.len().saturating_sub(start);
                        if count > MAX_EXPAND {
                            return Err(compile_err!(
                                "batch-impl: `@{}..` expands to {} predicates (max {})",
                                start,
                                count,
                                MAX_EXPAND
                            ));
                        }
                        let tail = resolve_tail(&tokens[i + 4..], impl_names)?;
                        emit_fresh_predicates(&mut out, &fresh_sorted[start..start + count], &tail);
                        i = tokens.len();
                        continue;
                    }
                    // `@N..M` / `@N..=M`: a contiguous fresh range — each
                    // indexed fresh gets the predicate tail (comma-separated).
                    // Out of range or over MAX_EXPAND predicates errors.
                    if let Ok(start) = s.parse::<usize>()
                        && matches!(tokens.get(i + 2), Some(TokenTree::Punct(p)) if p.as_char() == '.')
                        && matches!(tokens.get(i + 3), Some(TokenTree::Punct(p)) if p.as_char() == '.')
                    {
                        let (count, _end_idx, tail) =
                            parse_fresh_range(&tokens, i, start, fresh_sorted.len())?;
                        let tail = resolve_tail(&tail, impl_names)?;
                        emit_fresh_predicates(&mut out, &fresh_sorted[start..start + count], &tail);
                        i = tokens.len();
                        continue;
                    }
                    if let Ok(idx) = s.parse::<usize>() {
                        // Document-order index: `@N` resolves to the N-th fresh
                        // after (group, position) sorting — the same order the
                        // sweeper renumbers to `_Param_0..N_BatchGen_`.
                        let Some(&(_, _, name)) = fresh_sorted.get(idx) else {
                            return Err(at_num_out_of_range(
                                idx,
                                fresh_sorted.len(),
                                tokens[i].span(),
                            ));
                        };
                        out.extend(name.clone());
                        i += 2;
                        continue;
                    }
                    // `@g_i` (literal with an underscore): group g, position i
                    // of that group — resolves to the grouped fresh name
                    // `_Param_{g}_{i}_BatchGen_` (which the sweeper renumbers
                    // along with the generated names). Unlike `@N` it is
                    // stable across array-dispatch impls (a group absent from
                    // an impl errors here instead of silently shifting).
                    if let Some((g, pos)) = s.split_once('_')
                        && let (Ok(g), Ok(pos)) = (g.parse::<usize>(), pos.parse::<usize>())
                    {
                        let target = format!("_Param_{}_{}_BatchGen_", g, pos);
                        let Some(name) = impl_names.iter().find(|n| n.to_string() == target) else {
                            return Err(at_group_out_of_range(g, pos, tokens[i].span()));
                        };
                        out.extend(name.clone());
                        i += 2;
                        continue;
                    }
                    return Err(compile_error_str(
                        "batch-impl: `@` in a where predicate must be followed by \
                         a position digit (e.g. `@0` or `@0_1`)",
                        tokens[i].span(),
                    ));
                }
                _ => {
                    return Err(compile_error_str(
                        "batch-impl: `@` in a where predicate must be a position digit (e.g. `@0` or `@0_1`)",
                        tokens[i].span(),
                    ));
                }
            }
        } else if let TokenTree::Group(g) = &tokens[i] {
            // Recurse into groups (`Module<..., Scalar = @0::Scalar>` — the
            // angle group is paired by angle_collect; `@N` inside it is a
            // value reference that must resolve like the top level, mirroring
            // `parse::resolve_at_refs`).
            let inner = g.stream().into_iter().collect::<Vec<_>>();
            let resolved = resolve_tail(&inner, impl_names)?;
            let mut ng = Group::new(g.delimiter(), resolved.into_iter().collect());
            ng.set_span(g.span());
            out.push(TokenTree::Group(ng));
            i += 1;
        } else {
            out.push(tokens[i].clone());
            i += 1;
        }
    }
    Ok(out.into_iter().collect())
}

/// Resolves the `@` references in a predicate tail (the type position after
/// `:` — `@N` may appear inside angle groups, e.g. `Scalar = @0::Scalar`).
fn resolve_tail(
    tail: &[TokenTree], impl_names: &[TokenStream],
) -> Result<Vec<TokenTree>, TokenStream> {
    let ts = tail.iter().cloned().collect();
    resolve_where_at(&ts, impl_names).map(|r| r.into_iter().collect())
}

/// Emits `name0 tail, name1 tail, ...` (comma-separated) into `out` — the
/// single authority for the fresh-predicate emission shared by `@all_fresh`
/// and the `@N..M` range form.
fn emit_fresh_predicates(
    out: &mut Vec<TokenTree>, names: &[(usize, usize, &TokenStream)], tail: &[TokenTree],
) {
    let comma = TokenTree::Punct(Punct::new(',', Spacing::Alone));
    for (k, &(_, _, name)) in names.iter().enumerate() {
        if k > 0 {
            out.push(comma.clone());
        }
        out.extend(name.clone());
        out.extend(tail.iter().cloned());
    }
}

/// Parse the `@N..M` / `@N..=M` fresh-range subject (the `@N` and the `..`/
/// `..=` are already confirmed by the caller). Returns `(count, end_idx, tail)`
/// — `count` fresh names starting at `start`, the predicate tail after the
/// range, and the token index just past the range. All range checks (empty /
/// out-of-range / over `MAX_EXPAND`) error here.
fn parse_fresh_range(
    tokens: &[TokenTree], i: usize, start: usize, fresh_len: usize,
) -> Result<(usize, usize, Vec<TokenTree>), TokenStream> {
    let inclusive = matches!(tokens.get(i + 4), Some(TokenTree::Punct(p)) if p.as_char() == '=');
    let end_idx = if inclusive { i + 5 } else { i + 4 };
    let Some(TokenTree::Literal(end_lit)) = tokens.get(end_idx) else {
        return Err(compile_error_str(
            "batch-impl: a `@N..M` range in a where predicate must end with a number (e.g. `@0..=2`)",
            tokens[i].span(),
        ));
    };
    let Ok(end) = end_lit.to_string().parse::<usize>() else {
        return Err(compile_error_str(
            "batch-impl: a `@N..M` range in a where predicate must end with a number (e.g. `@0..=2`)",
            end_lit.span(),
        ));
    };
    let count = if inclusive { end.saturating_sub(start) + 1 } else { end.saturating_sub(start) };
    if count == 0 {
        return Err(compile_err!(
            "batch-impl: `@{}..{}` is an empty range (start not below end); no predicates will be generated",
            start,
            end
        ));
    }
    if end >= fresh_len || start > end {
        return Err(compile_err!(
            "batch-impl: `@{}..{}` out of range in a where predicate (impl has {} fresh generics, numbered from 0 in document order)",
            start,
            end,
            fresh_len
        ));
    }
    if count > MAX_EXPAND {
        return Err(compile_err!(
            "batch-impl: `@{}..{}` expands to {} predicates (max {})",
            start,
            end,
            count,
            MAX_EXPAND
        ));
    }
    let tail = tokens[end_idx + 1..].to_vec();
    Ok((count, end_idx, tail))
}

/// Parses a grouped range literal `L_N` (the part after `@`) into
/// `(group, start)`; `None` for a plain digit (that is the flat `@N` form).
fn parse_group_start(s: &str) -> Option<(usize, usize)> {
    let (l, n) = s.split_once('_')?;
    Some((l.parse::<usize>().ok()?, n.parse::<usize>().ok()?))
}
