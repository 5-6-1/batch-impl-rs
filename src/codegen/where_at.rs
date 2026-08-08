//! `@` position references in where predicates: `@N` / `@g_i` / `@all_fresh`
//! / `@N..M` resolve against the impl's fresh generics (document order for
//! `@N`, exact generating site for `@g_i`, batch forms for the rest).

use proc_macro2::{Punct, Spacing, TokenStream, TokenTree};

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
pub(crate) fn resolve_where_at(
    pred: &TokenStream, impl_names: &[TokenStream],
) -> Result<TokenStream, TokenStream> {
    // Fresh params sorted by (group, position) — the sweep order, so `@N`
    // matches the final `_Param_{N}_BatchGen_` the sweeper will emit.
    let mut fresh_sorted: Vec<&TokenStream> = impl_names
        .iter()
        .filter(|n| parse_grouped_fresh(&n.to_string()).is_some())
        .collect();
    fresh_sorted.sort_by_key(|n| parse_grouped_fresh(&n.to_string()).unwrap());
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
                    let tail = tokens[i + 2..].to_vec();
                    let comma = TokenTree::Punct(Punct::new(',', Spacing::Alone));
                    for (k, &name) in fresh_sorted.iter().enumerate() {
                        if k > 0 {
                            out.push(comma.clone());
                        }
                        out.extend(name.clone());
                        out.extend(tail.iter().cloned());
                    }
                    i = tokens.len();
                    continue;
                }
                Some(TokenTree::Literal(lit)) => {
                    let s = lit.to_string();
                    // `@N..M` / `@N..=M`: a contiguous fresh range — each
                    // indexed fresh gets the predicate tail (comma-separated).
                    // Out of range or over MAX_EXPAND predicates errors.
                    if let Ok(start) = s.parse::<usize>()
                        && matches!(tokens.get(i + 2), Some(TokenTree::Punct(p)) if p.as_char() == '.')
                        && matches!(tokens.get(i + 3), Some(TokenTree::Punct(p)) if p.as_char() == '.')
                    {
                        let inclusive = matches!(tokens.get(i + 4), Some(TokenTree::Punct(p)) if p.as_char() == '=');
                        let end_idx = if inclusive { i + 5 } else { i + 4 };
                        let Some(TokenTree::Literal(end_lit)) = tokens.get(end_idx)
                        else {
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
                        let count = if inclusive {
                            end.saturating_sub(start) + 1
                        } else {
                            end.saturating_sub(start)
                        };
                        if count == 0 {
                            return Err(compile_err!(
                                "batch-impl: `@{}..{}` is an empty range (start \
                                 not below end); no predicates will be generated",
                                start,
                                end
                            ));
                        }
                        if end >= fresh_sorted.len() || start > end {
                            return Err(compile_err!(
                                "batch-impl: `@{}..{}` out of range in a where \
                                 predicate (impl has {} fresh generics, numbered \
                                 from 0 in document order)",
                                start,
                                end,
                                fresh_sorted.len()
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
                        let comma = TokenTree::Punct(Punct::new(',', Spacing::Alone));
                        for (offset, &name) in
                            fresh_sorted[start..start + count].iter().enumerate()
                        {
                            if offset > 0 {
                                out.push(comma.clone());
                            }
                            out.extend(name.clone());
                            out.extend(tail.iter().cloned());
                        }
                        i = tokens.len();
                        continue;
                    }
                    if let Ok(idx) = s.parse::<usize>() {
                        // Document-order index: `@N` resolves to the N-th fresh
                        // after (group, position) sorting — the same order the
                        // sweeper renumbers to `_Param_0..N_BatchGen_`.
                        let Some(&name) = fresh_sorted.get(idx) else {
                            return Err(compile_err!(
                                "batch-impl: `@{}` out of range in a where predicate \
                                 (impl has {} fresh generics, numbered from 0 in \
                                 document order; user-written params are addressed \
                                 by name)",
                                idx,
                                fresh_sorted.len()
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
                        && let (Ok(g), Ok(pos)) =
                            (g.parse::<usize>(), pos.parse::<usize>())
                    {
                        let target = format!("_Param_{}_{}_BatchGen_", g, pos);
                        let Some(name) =
                            impl_names.iter().find(|n| n.to_string() == target)
                        else {
                            return Err(compile_err!(
                                "batch-impl: `@{}` in a where predicate — this \
                                 impl has no group {} position {} (grouped \
                                 fresh names are `_Param_{{g}}_{{i}}_BatchGen_`; \
                                 use `@N` for the impl's document-order fresh)",
                                s,
                                g,
                                pos
                            ));
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
        } else {
            out.push(tokens[i].clone());
            i += 1;
        }
    }
    Ok(out.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use crate::analyze::extract_trait_bounds;
    use crate::ast::*;
    use crate::codegen::generate_impl;
    use quote::quote;
    use syn::parse_quote;

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
                params: vec![(quote!(T), None), (quote!(N), None)],
                bindings: vec![],
            },
        );
        let wrapped = TyWithTrait(trait_ty, target.into());
        let impl_ty = TyWithType(
            TyTypeParam {
                params: vec![
                    (quote!(T), None),
                    (quote!(const N), Some(TyPrimitive(quote!(usize)).to_ty())),
                ],
                bindings: vec![],
            },
            wrapped.into(),
        )
        .into();
        let out =
            generate_impl(impl_ty, &quote!(WhereArr), false, &tb, &[]).to_string();
        assert!(
            !out.contains("compile_error"),
            "expansion must not contain compile_error: {out}"
        );
        assert!(
            out.contains("where [T ; N] : Sized"),
            "missing where predicate: {out}"
        );
        assert!(
            out.contains("impl < T , const N : usize > WhereArr < T , N >"),
            "unexpected impl generics: {out}"
        );
    }
}
