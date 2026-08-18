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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze::extract_trait_bounds;
    use crate::ast::*;
    use crate::codegen::generate_impl;
    use proc_macro2::Group;
    use quote::quote;
    use syn::parse_quote;

    fn fresh_names(n: usize) -> Vec<TokenStream> {
        (0..n).map(|i| format!("_Param_0_{}_BatchGen_", i).parse().unwrap()).collect()
    }

    fn resolve(s: &str, names: &[TokenStream]) -> String {
        let pred: TokenStream = s.parse().unwrap();
        resolve_where_at(&pred, names).unwrap().to_string()
    }

    #[test]
    fn open_range_from_second() {
        // `@1..` open range: every fresh from index 1 to the last one
        let names = fresh_names(4);
        assert_eq!(
            resolve("@1.. : Bound", &names),
            "_Param_0_1_BatchGen_ : Bound , _Param_0_2_BatchGen_ : Bound , \
             _Param_0_3_BatchGen_ : Bound"
        );
    }

    #[test]
    fn open_range_empty_when_past_end() {
        // arity 1: no "from the second element" predicate — the open range
        // truncates to zero instead of erroring (alga2's `@1..` requirement)
        let names = fresh_names(1);
        assert_eq!(resolve("@1.. : Bound", &names), "");
    }

    #[test]
    fn at_ref_inside_group_resolves() {
        // angle_collect pairs `<>` into a None group; `@0` inside is a value
        // reference and must resolve (recursion mirrors resolve_at_refs)
        let names = fresh_names(2);
        let inner: TokenStream = "Scalar = @0 :: Scalar".parse().unwrap();
        let none = Group::new(proc_macro2::Delimiter::None, inner);
        let pred = TokenStream::from(TokenTree::Group(none));
        assert_eq!(
            resolve_where_at(&pred, &names).unwrap().to_string(),
            "Scalar = _Param_0_0_BatchGen_ :: Scalar"
        );
    }

    #[test]
    fn range_tail_value_ref() {
        // the tail after a range subject is scanned for `@N` too (the
        // alga2 scenario: `Scalar = @0::Scalar` inside the bound)
        let names = fresh_names(3);
        let out = resolve("@1.. : Module < Scalar = @0 :: Scalar >", &names);
        assert_eq!(
            out,
            "_Param_0_1_BatchGen_ : Module < Scalar = _Param_0_0_BatchGen_ :: Scalar > , \
             _Param_0_2_BatchGen_ : Module < Scalar = _Param_0_0_BatchGen_ :: Scalar >"
        );
    }

    #[test]
    fn closed_range_tail_value_ref() {
        let names = fresh_names(3);
        let out = resolve("@1..=2 : Module < Scalar = @0 :: Scalar >", &names);
        assert_eq!(
            out,
            "_Param_0_1_BatchGen_ : Module < Scalar = _Param_0_0_BatchGen_ :: Scalar > , \
             _Param_0_2_BatchGen_ : Module < Scalar = _Param_0_0_BatchGen_ :: Scalar >"
        );
    }

    /// `WhereArr<>` expansion: impl generics `[T, const N: usize]` (parse-layer name is
    /// `const N`; the keyword is needed to render), trait args `[T, N]`, predicate
    /// `[T; N]: Sized` referencing N — after normalization the check passes and the
    /// expansion has no compile_error (regression guard against IDE/stale false positives)
    #[test]
    fn const_param_where_predicate_no_error() {
        let trait_def: syn::ItemTrait = parse_quote!(
            trait WhereArr<T, const N: usize>
            where
                [T; N]: Sized,
            {
            }
        );
        let tb = extract_trait_bounds(&trait_def);
        let target = TyTuple(vec![]).to_ty();
        let trait_ty = TyTrait(
            quote!(WhereArr),
            TyTypeParam {
                params: vec![
                    (Box::new(TyPrimitive(quote!(T)).to_ty()), None),
                    (Box::new(TyPrimitive(quote!(N)).to_ty()), None),
                ],
                bindings: vec![],
            },
        );
        let wrapped = TyWithTrait(trait_ty, target.into());
        let impl_ty = TyWithType(
            TyTypeParam {
                params: vec![
                    (Box::new(TyPrimitive(quote!(T)).to_ty()), None),
                    (
                        Box::new(TyPrimitive(quote!(const N)).to_ty()),
                        Some(TyPrimitive(quote!(usize)).to_ty()),
                    ),
                ],
                bindings: vec![],
            },
            wrapped.into(),
        )
        .into();
        let out = generate_impl(impl_ty, &quote!(WhereArr), false, &tb, &[]).to_string();
        assert!(!out.contains("compile_error"), "expansion must not contain compile_error: {out}");
        assert!(out.contains("where [T ; N] : Sized"), "missing where predicate: {out}");
        assert!(
            out.contains("impl < T , const N : usize > WhereArr < T , N >"),
            "unexpected impl generics: {out}"
        );
    }
}
