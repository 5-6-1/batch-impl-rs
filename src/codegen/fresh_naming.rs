//! Fresh-generic naming and `@` reference validation. The per-impl context
//! ([`FreshCtx`]) is built once in [`generate_parts`](crate::codegen::generate_parts)
//! after every fresh declaration has been hoisted: identity pairs are parsed
//! from the declaration carriers, sorted into document order, and each is
//! assigned its display name (`P0, P1, ...`) collision-aware against the
//! idents the impl already uses. Every macro-meta consumer (`where_at` /
//! `range_refs` / shape / repeat) resolves against these display names, so
//! nothing internal ever reaches rendered output unnamed. User-written params
//! do not participate (`@N` exists exactly because fresh names are
//! unknowable). The naming protocol itself lives in `crate::ast::fresh_protocol`.

use proc_macro2::{Ident, Span, TokenStream, TokenTree};
use quote::quote;
use std::collections::HashSet;

use crate::ast::Ty;
use crate::ast::fresh_protocol::decl_fresh_pos;
use crate::util::compile_error_str;

/// The per-impl macro-meta context: fresh declarations sorted by
/// (group, position) — exactly the document order users address with `@N` —
/// each paired with its final display name, so a reference resolves straight
/// to the name the rendered impl will declare.
pub(crate) struct FreshCtx {
    pub(crate) names: Vec<(usize, usize, TokenStream)>,
}

impl FreshCtx {
    /// Collects the fresh declarations of one impl from its generic name
    /// streams (declaration carriers; user-written params yield nothing),
    /// numbers them in document order and assigns display names. `used` is
    /// the impl's already-written idents — a display name taken by the impl
    /// escape with spreadsheet-style letters (`P1` → `P1A` → `P1B`; the
    /// numbering itself never skips), so the `@N` correspondence holds and
    /// every name stays lint-clean and readable.
    pub(crate) fn new(decl_names: &[TokenStream], used: &HashSet<String>) -> Self {
        let mut fresh = decl_names.iter().filter_map(decl_fresh_pos).collect::<Vec<_>>();
        fresh.sort_unstable();
        fresh.dedup();
        let names = fresh
            .into_iter()
            .enumerate()
            .map(|(k, gi)| {
                let id = Ident::new(&display_name(k, used), Span::call_site());
                (gi.0, gi.1, quote!(#id))
            })
            .collect();
        Self { names }
    }

    /// The entries of one generator group (sorted by position within the
    /// group); an unknown group errors (the single authority for that
    /// diagnostic — shared by the flat and predicate-subject resolvers).
    pub(crate) fn group(
        &self, group: usize, span: Span,
    ) -> Result<&[(usize, usize, TokenStream)], TokenStream> {
        let start = self.names.iter().position(|&(g, _, _)| g == group).ok_or_else(|| {
            compile_error_str(
                &format!(
                    "batch-impl: `@{}_..` group {} does not exist — this impl has \
                         no generator group {}",
                    group, group, group,
                ),
                span,
            )
        })?;
        let end = crate::util::slice_from(&self.names, start)
            .iter()
            .position(|&(g, _, _)| g != group)
            .map_or(self.names.len(), |p| start + p);
        Ok(crate::util::slice_between(&self.names, start, end))
    }
}

/// The display name of the k-th fresh (document order): `P{k}`, escaping a
/// taken name with **spreadsheet-style letters** — `P0` → `P0A` → `P0B` …
/// `P0Z` → `P0AA` (bijective base-26). No underscores: the escaped name
/// stays upper camel case (rustc lint-clean) and carries none of the
/// leading-`_` "intentionally unused" connotation. The numbering itself
/// never skips; each fresh has a distinct base, so escape sequences cannot
/// collide with one another.
pub(crate) fn display_name(k: usize, used: &HashSet<String>) -> String {
    let base = format!("P{}", k);
    let mut name = base.clone();
    let mut n = 0usize;
    while used.contains(&name) {
        n += 1;
        name = format!("{}{}", base, letters(n));
    }
    name
}

/// 1 → `A`, 2 → `B` … 26 → `Z`, 27 → `AA` (bijective base-26).
fn letters(mut n: usize) -> String {
    let mut out = Vec::new();
    while n > 0 {
        n -= 1;
        out.push((b'A' + (n % 26) as u8) as char);
        n /= 26;
    }
    out.into_iter().rev().collect()
}

/// Gathers every ident the impl already writes — the collision set display
/// names must skip. Declaration carriers hold only digits and punctuation,
/// so a plain walk over all groups cannot false-positive on them.
pub(crate) fn collect_used_idents(tokens: &TokenStream, used: &mut HashSet<String>) {
    for tt in tokens.clone() {
        match tt {
            TokenTree::Ident(id) => {
                used.insert(id.to_string());
            }
            TokenTree::Group(g) => collect_used_idents(&g.stream(), used),
            _ => {}
        }
    }
}

/// Feeds several surfaces into one collision set — the **single source-list
/// authority** for every display-name minting path. When each entry collected
/// an ad-hoc list of its own, the impl entry missed the matrix leaf and the
/// spec's where predicates that the attribute entry counted, so a fresh
/// display name could shadow a user ident appearing only there (the `R1`
/// drift class: two front-ends must agree on what "already used" means).
pub(crate) fn collect_used_surfaces(surfaces: &[&TokenStream], used: &mut HashSet<String>) {
    for s in surfaces {
        collect_used_idents(s, used);
    }
}

/// The collision set of [`collect_used_surfaces`], freshly built — the
/// constructor form for callers that own the whole list.
pub(crate) fn used_ident_set(surfaces: &[&TokenStream]) -> HashSet<String> {
    let mut used = HashSet::new();
    collect_used_surfaces(surfaces, &mut used);
    used
}

/// Renames every declaration carrier in the impl-generic list to its display
/// name — the last place a raw carrier exists on the impl path. Entries that
/// are not declaration carriers (user params, `const` params) pass through.
pub(crate) fn rename_fresh_decls(impl_generics: &mut [(TokenStream, Option<Ty>)], ctx: &FreshCtx) {
    for (name, _) in impl_generics.iter_mut() {
        if let Some(key) = decl_fresh_pos(name)
            && let Some(entry) = ctx.names.iter().find(|(g, i, _)| (*g, *i) == key)
        {
            *name = entry.2.clone();
        }
    }
}
