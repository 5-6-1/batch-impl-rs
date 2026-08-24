//! Fresh-name sweeping and `@` reference validation: renumbers
//! grouped fresh names (`_Param_{g}_{i}_BatchGen_`) to `_Param_0..N_BatchGen_`
//! per impl so `@N` is a pure construction, and validates `@N` / `@g_i`
//! references that survived into the target type / trait args (the where-
//! predicate positions are validated by `resolve_where_at`). The naming
//! protocol itself lives in `crate::ast::fresh`.

use proc_macro2::{Group, Ident, Span, TokenStream, TokenTree};
use quote::ToTokens;

use crate::ast::fresh::{FreshEnd, FreshRef};
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

/// The per-impl fresh-generic context, built once in [`generate_parts`] and
/// shared by every macro-meta consumer (`where_at` / `range_refs` / shape /
/// repeat): grouped fresh names sorted by (group, position) — exactly the
/// document order the sweeper renumbers to `_Param_0..N_BatchGen_`, so `@N`
/// indexes straight into [`FreshCtx::names`]. User-written params do not
/// participate (`@N` exists exactly because fresh names are unknowable).
pub(crate) struct FreshCtx<'a> {
    pub(crate) names: Vec<(usize, usize, &'a TokenStream)>,
}

impl<'a> FreshCtx<'a> {
    /// Collects and sorts the grouped fresh names of one impl.
    pub(crate) fn new(impl_names: &'a [TokenStream]) -> Self {
        let mut names: Vec<(usize, usize, &TokenStream)> = impl_names
            .iter()
            .filter_map(|n| {
                let (g, i) = parse_grouped_fresh(&n.to_string())?;
                Some((g, i, n))
            })
            .collect();
        names.sort_by_key(|&(g, i, _)| (g, i));
        Self { names }
    }

    /// The entries of one generator group (sorted by position within the
    /// group); an unknown group errors (the single authority for that
    /// diagnostic — shared by the flat and predicate-subject resolvers).
    pub(crate) fn group(
        &self, group: usize, span: Span,
    ) -> Result<&[(usize, usize, &TokenStream)], TokenStream> {
        let start =
            self.names.iter().position(|&(g, _, _)| g == group).ok_or_else(|| {
                compile_error_str(
                    &format!(
                        "batch-impl: `@{}_..` group {} does not exist — this impl has \
                         no generator group {}",
                        group, group, group,
                    ),
                    span,
                )
            })?;
        let end = self.names[start..]
            .iter()
            .position(|&(g, _, _)| g != group)
            .map_or(self.names.len(), |p| start + p);
        Ok(&self.names[start..end])
    }
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

/// Validates `@{...}` references that survived into the target type or
/// the trait args (where predicates are validated by `resolve_where_at`): a
/// reference outside the impl's fresh list is dangling — report it in user
/// language instead of leaking the reserved `_Param_*_BatchGen_` name into
/// rustc's E0412 output.
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

/// Recursive token walk: a carrier `@{...}` must be within the impl's fresh
/// list — a single position indexes it, a grouped form must exist, a range
/// end must be below the count (an open range never dangles: it truncates).
fn collect_dangling(
    tokens: TokenStream, declared: &std::collections::HashSet<(usize, usize)>, fresh_count: usize,
) -> Vec<TokenStream> {
    let v: Vec<_> = tokens.into_iter().collect();
    let mut errs = vec![];
    let mut i = 0;
    while i < v.len() {
        if is_fresh_carrier(&v[i], v.get(i + 1)) {
            let span = match &v[i] {
                TokenTree::Punct(p) => p.span(),
                _ => Span::call_site(),
            };
            if let Some(TokenTree::Group(g)) = v.get(i + 1) {
                let inner: String =
                    g.stream().into_iter().map(|t| t.to_string()).collect::<Vec<_>>().join("");
                if let Some(r) = FreshRef::parse(&inner) {
                    errs.extend(validate_ref(&r, declared, fresh_count, span));
                }
            }
            i += 2;
            continue;
        }
        // Declarations themselves are validated by construction; recurse.
        if let TokenTree::Group(g) = &v[i] {
            errs.extend(collect_dangling(g.stream(), declared, fresh_count));
        }
        i += 1;
    }
    errs
}

/// Whether a token pair is a fresh-ref carrier: a `@` punct directly
/// followed by a Brace group.
fn is_fresh_carrier(at: &TokenTree, g: Option<&TokenTree>) -> bool {
    matches!(at, TokenTree::Punct(p) if p.as_char() == '@')
        && matches!(g, Some(TokenTree::Group(g)) if g.delimiter() == proc_macro2::Delimiter::Brace)
}

/// The range/single checks shared by every validator — one authority so the
/// wording and the bounds cannot drift apart between positions.
fn validate_ref(
    r: &FreshRef, declared: &std::collections::HashSet<(usize, usize)>, fresh_count: usize,
    span: Span,
) -> Vec<TokenStream> {
    // Grouped form: the group must exist, then the extent must fit its slice.
    if let Some(g) = r.group {
        let len = declared.iter().filter(|&&(gg, _)| gg == g).count();
        if len == 0 {
            return vec![at_group_out_of_range(g, r.start, span)];
        }
        let fits = match r.end {
            FreshEnd::Single => r.start < len,
            FreshEnd::Open => true,
            FreshEnd::Closed(e) => e < len && r.start <= e,
        };
        return if fits {
            vec![]
        } else {
            vec![compile_error_str(
                &format!(
                    "batch-impl: `{}_{}` out of range — generator group {} has {} fresh generics",
                    g,
                    r.spell(),
                    g,
                    len
                ),
                span,
            )]
        };
    }
    // Flat form: index against the whole fresh list.
    let fits = match r.end {
        FreshEnd::Single => r.start < fresh_count,
        FreshEnd::Open => true, // an open range past the end truncates to empty
        FreshEnd::Closed(e) => e < fresh_count && r.start <= e,
    };
    if fits {
        return vec![];
    }
    match r.end {
        FreshEnd::Single => vec![at_num_out_of_range(r.start, fresh_count, span)],
        _ => vec![compile_error_str(
            &format!(
                "batch-impl: `@{}` out of range — this scope has {} fresh \
                 generics (numbered from 0 in document order)",
                r.spell(),
                fresh_count,
            ),
            span,
        )],
    }
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
