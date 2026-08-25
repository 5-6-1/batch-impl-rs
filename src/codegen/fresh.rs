//! Fresh-generic naming and `@` reference validation. The per-impl context
//! ([`FreshCtx`]) is built once in [`generate_parts`](crate::codegen::generate_parts)
//! after every fresh declaration has been hoisted: identity pairs are parsed
//! from the declaration carriers, sorted into document order, and each is
//! assigned its display name (`P0, P1, ...`) collision-aware against the
//! idents the impl already uses. Every macro-meta consumer (`where_at` /
//! `range_refs` / shape / repeat) resolves against these display names, so
//! nothing internal ever reaches rendered output unnamed. User-written params
//! do not participate (`@N` exists exactly because fresh names are
//! unknowable). The naming protocol itself lives in `crate::ast::fresh`.

use proc_macro2::{Group, Ident, Span, TokenStream, TokenTree};
use quote::{ToTokens, quote};
use std::collections::{HashMap, HashSet};

use crate::ast::Ty;
use crate::ast::fresh::{FreshEnd, FreshRef, decl_fresh_pos};
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
        let mut fresh: Vec<(usize, usize)> = decl_names.iter().filter_map(decl_fresh_pos).collect();
        fresh.sort_unstable();
        fresh.dedup();
        let display: HashMap<(usize, usize), String> =
            fresh.iter().enumerate().map(|(k, &gi)| (gi, display_name(k, used))).collect();
        let names = fresh
            .into_iter()
            .map(|gi| {
                let id = Ident::new(&display[&gi], Span::call_site());
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
        let end = self.names[start..]
            .iter()
            .position(|&(g, _, _)| g != group)
            .map_or(self.names.len(), |p| start + p);
        Ok(&self.names[start..end])
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

/// Final naming for the **top-level macro form**: spec tokens prepended to an
/// external macro input may carry declaration/reference carriers — number
/// them by document order and rewrite straight to display names, escaping
/// idents the stream already writes by the same underscore-prefix rule as
/// the impl path. A stream without carriers passes through unchanged.
pub(crate) fn finalize_fresh_names(tokens: TokenStream) -> TokenStream {
    let v: Vec<_> = tokens.into_iter().collect();
    let mut groups: Vec<(usize, usize)> = vec![];
    collect_carriers(&v, &mut groups);
    if groups.is_empty() {
        return v.into_iter().collect();
    }
    let mut used = HashSet::new();
    collect_used_idents(&v.iter().cloned().collect::<TokenStream>(), &mut used);
    groups.sort_unstable();
    groups.dedup();
    let map: HashMap<(usize, usize), String> =
        groups.iter().enumerate().map(|(k, &gi)| (gi, display_name(k, &used))).collect();
    rewrite_carriers(v, &map).into_iter().collect()
}

/// One walk gathering the carrier identities of the stream (carrier groups
/// are atomic and hold no nested carriers — not descended).
fn collect_carriers(v: &[TokenTree], groups: &mut Vec<(usize, usize)>) {
    let mut i = 0;
    while i < v.len() {
        match (&v[i], v.get(i + 1)) {
            (TokenTree::Punct(p), Some(TokenTree::Group(g)))
                if p.as_char() == '@' && g.delimiter() == proc_macro2::Delimiter::Brace =>
            {
                let inner: String =
                    g.stream().into_iter().map(|t| t.to_string()).collect::<Vec<_>>().join("");
                if let Some(FreshRef { group: Some(gp), start, end: FreshEnd::Single }) =
                    FreshRef::parse(&inner)
                {
                    groups.push((gp, start));
                }
                i += 2;
            }
            (TokenTree::Group(g), _) => {
                let inner: Vec<_> = g.stream().into_iter().collect();
                collect_carriers(&inner, groups);
                i += 1;
            }
            _ => i += 1,
        }
    }
}

fn rewrite_carriers(v: Vec<TokenTree>, map: &HashMap<(usize, usize), String>) -> Vec<TokenTree> {
    let mut out = vec![];
    let mut i = 0;
    while i < v.len() {
        match (&v[i], v.get(i + 1)) {
            (TokenTree::Punct(p), Some(TokenTree::Group(g)))
                if p.as_char() == '@' && g.delimiter() == proc_macro2::Delimiter::Brace =>
            {
                let inner: String =
                    g.stream().into_iter().map(|t| t.to_string()).collect::<Vec<_>>().join("");
                let name = FreshRef::parse(&inner)
                    .and_then(|r| match r {
                        FreshRef { group: Some(gp), start, end: FreshEnd::Single } => {
                            map.get(&(gp, start)).cloned()
                        }
                        _ => None,
                    })
                    .map(|n| {
                        let id = Ident::new(&n, p.span());
                        TokenTree::Ident(id)
                    });
                match name {
                    Some(id) => out.push(id),
                    None => {
                        out.push(v[i].clone());
                        out.push(v[i + 1].clone());
                    }
                }
                i += 2;
            }
            (TokenTree::Group(g), _) => {
                let inner: Vec<_> = g.stream().into_iter().collect();
                let mut ng =
                    Group::new(g.delimiter(), rewrite_carriers(inner, map).into_iter().collect());
                ng.set_span(g.span());
                out.push(TokenTree::Group(ng));
                i += 1;
            }
            _ => {
                out.push(v[i].clone());
                i += 1;
            }
        }
    }
    out
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
/// language instead of leaking an internal carrier into rustc's E0412 output.
pub(crate) fn validate_at_refs(
    target: &Ty, trait_args: &[TokenStream], ctx: &FreshCtx,
) -> Vec<TokenStream> {
    let declared: HashSet<(usize, usize)> = ctx.names.iter().map(|&(g, i, _)| (g, i)).collect();
    let tokens = std::iter::once(target.to_token_stream())
        .chain(trait_args.iter().cloned())
        .collect::<TokenStream>();
    collect_dangling(tokens, &declared, ctx.names.len())
}

/// Recursive token walk: a carrier `@{...}` must be within the impl's fresh
/// list — a single position indexes it, a grouped form must exist, a range
/// end must be below the count (an open range never dangles: it truncates).
fn collect_dangling(
    tokens: TokenStream, declared: &HashSet<(usize, usize)>, fresh_count: usize,
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
    r: &FreshRef, declared: &HashSet<(usize, usize)>, fresh_count: usize, span: Span,
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
    use crate::ast::TyFresh;
    use crate::ast::fresh::{fresh_decl_tokens, fresh_ref_tokens};
    use quote::quote;

    fn decl(g: usize, i: usize) -> TokenStream {
        fresh_decl_tokens(g, i)
    }

    #[test]
    fn readable_basic() {
        let used: HashSet<String> = ["Tr", "Box"].iter().map(|s| s.to_string()).collect();
        let ctx = FreshCtx::new(&[decl(0, 0)], &used);
        assert_eq!(
            ctx.names.iter().map(|n| n.2.to_string()).collect::<Vec<_>>(),
            vec!["P0".to_string()]
        );
    }

    #[test]
    fn readable_multiple_indexed_by_doc_order() {
        let ctx = FreshCtx::new(&[decl(1, 0), decl(0, 0), decl(1, 1)], &HashSet::new());
        let got: Vec<String> = ctx.names.iter().map(|n| n.2.to_string()).collect();
        assert_eq!(got, ["P0", "P1", "P2"]);
        // Document order is (group, position), not minting order.
        assert_eq!(
            ctx.names.iter().map(|n| (n.0, n.1)).collect::<Vec<_>>(),
            vec![(0, 0), (1, 0), (1, 1)]
        );
    }

    #[test]
    fn readable_skips_collisions() {
        // a user ident `P0` escapes that fresh to `P0A`; the numbering stays
        let used: HashSet<String> = ["P0"].iter().map(|s| s.to_string()).collect();
        let ctx = FreshCtx::new(&[decl(0, 0)], &used);
        assert_eq!(ctx.names[0].2.to_string(), "P0A");
    }

    #[test]
    fn readable_escapes_repeatedly() {
        // `P1` and `P1A` both taken → the second fresh escapes to `P1B`
        // while the first keeps its untouched base (`P0`).
        let used: HashSet<String> = ["P1", "P1A"].iter().map(|s| s.to_string()).collect();
        let ctx = FreshCtx::new(&[decl(0, 0), decl(1, 0)], &used);
        assert_eq!(ctx.names[0].2.to_string(), "P0");
        assert_eq!(ctx.names[1].2.to_string(), "P1B");
    }

    #[test]
    fn finalize_rewrites_carriers_everywhere() {
        let t = decl(0, 0);
        let u = decl(0, 1);
        let ts = quote! { impl<#t, #u> Tr for (#t, #u) where #t: Clone };
        assert_eq!(
            finalize_fresh_names(ts).to_string(),
            "impl < P0 , P1 > Tr for (P0 , P1) where P0 : Clone"
        );
    }

    #[test]
    fn finalize_leaves_plain_streams() {
        let ts: TokenStream = quote! { impl<T> Tr for Box<T> };
        assert_eq!(finalize_fresh_names(ts).to_string(), "impl < T > Tr for Box < T >");
    }

    #[test]
    fn dangling_single_ref_reports_in_user_language() {
        let ctx = FreshCtx::new(&[decl(0, 0)], &HashSet::new());
        let target: Ty = TyFresh(FreshRef { group: None, start: 3, end: FreshEnd::Single }).to_ty();
        let errs = validate_at_refs(&target, &[], &ctx);
        assert_eq!(errs.len(), 1);
        assert!(errs[0].to_string().contains("out of range"), "{}", errs[0]);
    }

    #[test]
    fn open_range_ref_never_dangles() {
        let ctx = FreshCtx::new(&[decl(0, 0)], &HashSet::new());
        let target: Ty = TyFresh(FreshRef { group: None, start: 9, end: FreshEnd::Open }).to_ty();
        assert!(validate_at_refs(&target, &[], &ctx).is_empty());
        // A closed range past the end does dangle.
        let closed: Ty =
            TyFresh(FreshRef { group: None, start: 0, end: FreshEnd::Closed(3) }).to_ty();
        assert_eq!(validate_at_refs(&closed, &[], &ctx).len(), 1);
        let _ = fresh_ref_tokens; // exercised through the other suites
    }
}
