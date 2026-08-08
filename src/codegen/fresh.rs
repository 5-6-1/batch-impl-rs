//! Fresh-name sweeping: renumbers grouped fresh names
//! (`_Param_{g}_{i}_BatchGen_`) to `_Param_0..N_BatchGen_` per impl so `@N`
//! is a pure construction. The naming protocol itself lives in
//! `crate::ast::fresh`.

use proc_macro2::{Group, Ident, TokenStream, TokenTree};

use crate::ast::parse_grouped_fresh;

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

pub(crate) fn collect_grouped_fresh(
    tokens: &TokenStream, out: &mut Vec<(usize, usize)>,
) {
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
                if let Some(&k) = parse_grouped_fresh(&s).and_then(|gi| map.get(&gi))
                {
                    let name = format!("_Param_{}_BatchGen_", k);
                    out.push(TokenTree::Ident(Ident::new(&name, id.span())));
                } else {
                    out.push(TokenTree::Ident(id));
                }
            }
            TokenTree::Group(g) => {
                let inner = g.stream();
                let mut new_g =
                    Group::new(g.delimiter(), replace_grouped_fresh(inner, map));
                new_g.set_span(g.span());
                out.push(TokenTree::Group(new_g));
            }
            other => out.push(other),
        }
    }
    out.into_iter().collect()
}
