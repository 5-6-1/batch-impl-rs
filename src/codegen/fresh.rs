//! Fresh-name sweeping and `@` reference validation: renumbers
//! grouped fresh names (`_Param_{g}_{i}_BatchGen_`) to `_Param_0..N_BatchGen_`
//! per impl so `@N` is a pure construction, and validates `@N` / `@g_i`
//! references that survived into the target type / trait args (the where-
//! predicate positions are validated by `resolve_where_at`). The naming
//! protocol itself lives in `crate::ast::fresh`.

use proc_macro2::{Group, Ident, Span, TokenStream, TokenTree};
use quote::ToTokens;

use crate::ast::{Ty, parse_grouped_fresh, parse_numbered_fresh};
use crate::util::compile_error_str;

/// Sweeps grouped fresh names (`_Param_{g}_{i}_BatchGen_`) in a rendered
/// impl: renumbers them by (group, position) order to `_Param_0..N_BatchGen_`
/// (per impl — independent impls may reuse the same final names). The
/// grouping decouples generation (per-spec group ids) from the final `@N`
/// numbering: `@N` → `_Param_{N}_BatchGen_` is a pure construction and always
/// matches the swept name of the impl's N-th fresh in document order. Names
/// that do not match the grouped form pass through (user-written names or the
/// single-numbered `@N`-constructed ones). Returns the input unchanged when
/// no grouped fresh names exist.
pub(crate) fn sweep_fresh_names(tokens: TokenStream) -> TokenStream {
    let mut groups: Vec<(usize, usize)> = vec![];
    collect_grouped_fresh(&tokens, &mut groups);
    if groups.is_empty() {
        return tokens;
    }
    groups.sort_unstable();
    groups.dedup();
    let map: std::collections::HashMap<(usize, usize), usize> =
        groups.iter().enumerate().map(|(k, &gi)| (gi, k)).collect();
    replace_grouped_fresh(tokens, &map)
}

pub(crate) fn collect_grouped_fresh(tokens: &TokenStream, out: &mut Vec<(usize, usize)>) {
    for tt in tokens.clone() {
        match tt {
            TokenTree::Ident(id) => {
                if let Some(gi) = parse_grouped_fresh(&id.to_string()) {
                    out.push(gi);
                }
            }
            TokenTree::Group(g) => {
                let inner = g.stream();
                collect_grouped_fresh(&inner, out);
            }
            _ => {}
        }
    }
}

pub(crate) fn replace_grouped_fresh(
    tokens: TokenStream, map: &std::collections::HashMap<(usize, usize), usize>,
) -> TokenStream {
    let mut out = vec![];
    for tt in tokens {
        match tt {
            TokenTree::Ident(id) => {
                let s = id.to_string();
                if let Some(&k) = parse_grouped_fresh(&s).and_then(|gi| map.get(&gi)) {
                    let name = format!("_Param_{}_BatchGen_", k);
                    out.push(TokenTree::Ident(Ident::new(&name, id.span())));
                } else {
                    out.push(TokenTree::Ident(id));
                }
            }
            TokenTree::Group(g) => {
                let inner = g.stream();
                let mut new_g = Group::new(g.delimiter(), replace_grouped_fresh(inner, map));
                new_g.set_span(g.span());
                out.push(TokenTree::Group(new_g));
            }
            other => out.push(other),
        }
    }
    out.into_iter().collect()
}

/// Renames the swept fresh generics (`_Param_0..N_BatchGen_`) to the
/// readable `P0, P1, ...` scheme — P = Param, the index matches `@N`, so the
/// generated code is self-documenting (`impl<P0,P1> RangeSugar for (P0,P1)`
/// — the spelling the tutorial already uses). Runs after the sweep (the last
/// render step), so every internal protocol (`@N` construction, where
/// resolution, the sweep itself) sees the reserved names unchanged; the
/// rename is a pure presentation layer.
///
/// Collisions are skipped per fresh: an identifier already in use in the
/// impl block (a user generic named `P0`, a type named `P1`) pushes that
/// fresh to `P{n}_` — the numbering never drifts, so `@N` correspondence
/// stays stable.
pub(crate) fn readable_fresh_names(tokens: TokenStream) -> TokenStream {
    use std::collections::{HashMap, HashSet};
    let mut used: HashSet<String> = HashSet::new();
    collect_nonfresh_idents(&tokens, &mut used);
    let mut map: HashMap<String, Ident> = HashMap::new();
    let mut next = 0usize;
    rename_numbered_fresh(tokens, &mut map, &mut next, &used, 0)
}

fn collect_nonfresh_idents(tokens: &TokenStream, out: &mut std::collections::HashSet<String>) {
    for tt in tokens.clone() {
        match tt {
            TokenTree::Ident(id) => {
                let s = id.to_string();
                if !crate::ast::fresh::is_fresh_name(&s) {
                    out.insert(s);
                }
            }
            TokenTree::Group(g) => {
                let inner = g.stream();
                collect_nonfresh_idents(&inner, out);
            }
            _ => {}
        }
    }
}

fn rename_numbered_fresh(
    tokens: TokenStream, map: &mut std::collections::HashMap<String, Ident>, next: &mut usize,
    used: &std::collections::HashSet<String>, depth: usize,
) -> TokenStream {
    if depth > crate::util::MAX_NEST_DEPTH {
        return tokens;
    }
    let mut out = vec![];
    for tt in tokens {
        match tt {
            TokenTree::Ident(id) => {
                let s = id.to_string();
                if parse_numbered_fresh(&s).is_some() {
                    let name = map.entry(s).or_insert_with(|| {
                        let base = format!("P{}", *next);
                        *next += 1;
                        let final_name =
                            if used.contains(&base) { format!("{}_", base) } else { base };
                        Ident::new(&final_name, id.span())
                    });
                    out.push(TokenTree::Ident(name.clone()));
                } else {
                    out.push(TokenTree::Ident(id));
                }
            }
            TokenTree::Group(g) => {
                let inner = g.stream();
                let mut new_g = Group::new(
                    g.delimiter(),
                    rename_numbered_fresh(inner, map, next, used, depth + 1),
                );
                new_g.set_span(g.span());
                out.push(TokenTree::Group(new_g));
            }
            other => out.push(other),
        }
    }
    out.into_iter().collect()
}

/// `@N` out of range: the impl has fewer fresh generics than the index.
/// The single authority for this diagnostic — `resolve_where_at` (where
/// predicates) and [`validate_at_refs`] (target type / trait args) share it,
/// so the wording cannot drift apart.
pub(crate) fn at_num_out_of_range(n: usize, fresh_count: usize, span: Span) -> TokenStream {
    compile_error_str(
        &format!(
            "batch-impl: `@{}` is out of range — this impl has {} fresh \
             generics (numbered from 0 in document order; user-written params \
             are addressed by name)",
            n, fresh_count,
        ),
        span,
    )
}

/// `@g_i` references a group/position this impl never generated. The single
/// authority for this diagnostic — shared by [`validate_at_refs`] and the
/// where-predicate branch of `resolve_where_at`. The displayed `@{}_{}`
/// form is derived from the parsed pair, so it can never drift from the
/// values being reported.
pub(crate) fn at_group_out_of_range(g: usize, pos: usize, span: Span) -> TokenStream {
    compile_error_str(
        &format!(
            "batch-impl: `@{}_{}` does not match a generated generic — this impl \
             has no group {} position {} (groups and positions number from 0); \
             use `@N` for the N-th fresh generic in document order",
            g, pos, g, pos,
        ),
        span,
    )
}

/// Validates `@N` / `@g_i` references that survived into the target type or
/// the trait args (where predicates are validated by `resolve_where_at`): a
/// constructed fresh name not among the impl's declared generics is a dangling
/// reference — report it in user language instead of leaking the reserved
/// `_Param_*_BatchGen_` name into rustc's E0412 output.
pub(crate) fn validate_at_refs(
    target: &Ty, trait_args: &[TokenStream], impl_names: &[TokenStream],
) -> Vec<TokenStream> {
    let declared = impl_names
        .iter()
        .filter_map(|n| parse_grouped_fresh(&n.to_string()))
        .collect::<std::collections::HashSet<_>>();
    let tokens = std::iter::once(target.to_token_stream())
        .chain(trait_args.iter().cloned())
        .collect::<TokenStream>();
    collect_dangling(tokens, &declared, declared.len())
}

/// Recursive token walk: a grouped name must be declared; a single-numbered
/// `@N`-constructed name must be within the fresh count.
fn collect_dangling(
    tokens: TokenStream, declared: &std::collections::HashSet<(usize, usize)>, fresh_count: usize,
) -> Vec<TokenStream> {
    tokens
        .into_iter()
        .flat_map(|tt| match tt {
            TokenTree::Ident(id) => {
                let s = id.to_string();
                if let Some((g, pos)) = parse_grouped_fresh(&s) {
                    (!declared.contains(&(g, pos)))
                        .then(|| at_group_out_of_range(g, pos, id.span()))
                        .into_iter()
                        .collect()
                } else if let Some(n) = parse_numbered_fresh(&s) {
                    (n >= fresh_count)
                        .then(|| at_num_out_of_range(n, fresh_count, id.span()))
                        .into_iter()
                        .collect()
                } else {
                    vec![]
                }
            }
            TokenTree::Group(g) => collect_dangling(g.stream(), declared, fresh_count),
            _ => vec![],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn readable_basic() {
        let ts = quote! { impl<_Param_0_BatchGen_> Tr for Box<_Param_0_BatchGen_> };
        assert_eq!(readable_fresh_names(ts).to_string(), "impl < P0 > Tr for Box < P0 >");
    }

    #[test]
    fn readable_multiple_indexed() {
        let ts = quote! { impl<_Param_0_BatchGen_, _Param_1_BatchGen_> Tr for (_Param_0_BatchGen_, _Param_1_BatchGen_) };
        assert_eq!(readable_fresh_names(ts).to_string(), "impl < P0 , P1 > Tr for (P0 , P1)");
    }

    #[test]
    fn readable_skips_collisions() {
        // a user ident `P0` pushes that fresh to `P0_`; the numbering stays
        let ts = quote! { impl<_Param_0_BatchGen_> Tr for Box<P0> where _Param_0_BatchGen_: Sized };
        assert_eq!(
            readable_fresh_names(ts).to_string(),
            "impl < P0_ > Tr for Box < P0 > where P0_ : Sized"
        );
    }

    #[test]
    fn readable_leaves_nonfresh() {
        let ts = quote! { impl<T> Tr for Box<T> };
        assert_eq!(readable_fresh_names(ts).to_string(), "impl < T > Tr for Box < T >");
    }
}
