//! Top-level macro injection (`{! ...}`): the open-extension product / manual
//! attach form is recognized on a `WithCode` chain, the spec body (target +
//! preceding blocks in chain order) is merged into one Brace group, and the
//! macro call is rewritten to `name!{ {spec}(args){body}trait }`.

use proc_macro2::{Group, Ident, TokenStream, TokenTree};
use quote::quote;
use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::codegen::fresh::{collect_used_idents, display_name};
use crate::util::compile_error_str;

/// Detects the top-level macro form: a `WithCode` chain ending in a
/// `{! ...}` block (`!` as the block's first token — the open-extension
/// product or a user-written `T {! m!{...}}`). Returns the spec body tokens
/// (target type + preceding blocks in chain order, rendered) and the macro
/// call after the `!`. A `{!}` block must be the last block and there can
/// be at most one.
pub(crate) fn top_level_macro(ty: &Ty) -> Option<Result<(TokenStream, TokenStream), TokenStream>> {
    let mut body = vec![];
    let mut top = None;
    match walk_top_level(ty, &mut body, &mut top) {
        Ok(()) => top.map(|mac| Ok((body.into_iter().collect(), mac))),
        Err(e) => Some(Err(e)),
    }
}

pub(crate) fn walk_top_level(
    ty: &Ty, body: &mut Vec<TokenTree>, top: &mut Option<TokenStream>,
) -> Result<(), TokenStream> {
    match &ty.kind {
        TyKind::WithCode(TyWithCode(inner, code)) => {
            let tokens = code.0.clone().into_iter().collect::<Vec<TokenTree>>();
            let is_top = matches!(tokens.first(), Some(TokenTree::Punct(p)) if p.as_char() == '!');
            if is_top {
                if top.is_some() {
                    return Err(compile_error_str(
                        "batch-impl: at most one top-level `{! ...}` block per spec",
                        tokens.first().map_or_else(proc_macro2::Span::call_site, |t| t.span()),
                    ));
                }
                *top = Some(tokens.into_iter().skip(1).collect());
                // Still walk the inner chain: a `{!}` nested inside would be a
                // second top-level block (error above); a plain block after
                // the `{!}` is impossible (the `{!}` is the outermost).
                if let Some(inner) = inner {
                    walk_top_level(inner, body, top)?;
                }
            } else {
                // Plain block: legal when it is a *preceding* block (the
                // `{!}` sits further out, i.e. the chain tail) — `T {b} {! m!{...}}`.
                // Illegal only when a `{!}` was found *inside* this block
                // (the `{!}` would not be the last block) — `T {! m!{...}} {b}`.
                let top_before = top.is_some();
                if let Some(inner) = inner {
                    walk_top_level(inner, body, top)?;
                }
                if top.is_some() && !top_before {
                    return Err(compile_error_str(
                        "batch-impl: a `{! ...}` top-level block must be the last block",
                        tokens.first().map_or_else(proc_macro2::Span::call_site, |t| t.span()),
                    ));
                }
                body.extend(code.0.clone());
            }
        }
        _ => body.extend(render_ty_tokens(ty)),
    }
    Ok(())
}

/// Renders a non-WithCode Ty to tokens (the spec body's target type part).
pub(crate) fn render_ty_tokens(ty: &Ty) -> Vec<TokenTree> {
    quote!(#ty).into_iter().collect()
}

/// Prepends the spec body (as a single Brace group) to the macro call's
/// input group: `name!{ (args){body} trait }` →
/// `name!{ {spec} (args){body} trait }` (the spec group goes *inside* the
/// macro input group, right after the opening delimiter).
pub(crate) fn rewrite_macro_input(mac: TokenStream, spec: TokenStream) -> TokenStream {
    let tokens = mac.into_iter().collect::<Vec<TokenTree>>();
    let mut out = Vec::with_capacity(tokens.len() + 1);
    let mut inserted = false;
    let mut i = 0;
    while i < tokens.len() {
        if !inserted
            && matches!(&tokens[i], TokenTree::Punct(p) if p.as_char() == '!')
            && let Some(TokenTree::Group(g)) = tokens.get(i + 1)
        {
            out.push(tokens[i].clone()); // `!`
            // The spec body becomes the first *group* of the macro input
            // (`{spec}`) — the 4-segment protocol expects a Brace group.
            let spec_group = Group::new(delimiter![{}], spec.clone());
            let mut inner = TokenStream::new();
            inner.extend(std::iter::once(TokenTree::Group(spec_group)));
            inner.extend(g.stream());
            let mut new_g = Group::new(g.delimiter(), inner);
            new_g.set_span(g.span());
            out.push(TokenTree::Group(new_g));
            inserted = true;
            i += 2;
            continue;
        }
        out.push(tokens[i].clone());
        i += 1;
    }
    out.into_iter().collect()
}

pub(crate) fn finalize_fresh_names(tokens: TokenStream) -> TokenStream {
    let v = tokens.into_iter().collect::<Vec<_>>();
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
            _ if is_carrier_at(v, i) => {
                let TokenTree::Group(g) = &v[i + 1] else { unreachable!("matched above") };
                let inner = carrier_inner(g);
                if let Some(FreshRef { group: Some(gp), start, end: FreshEnd::Single }) =
                    FreshRef::parse(&inner)
                {
                    groups.push((gp, start));
                }
                i += 2;
            }
            (TokenTree::Group(g), _) => {
                let inner = g.stream().into_iter().collect::<Vec<_>>();
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
            _ if is_carrier_at(&v, i) => {
                let TokenTree::Group(g) = &v[i + 1] else { unreachable!("matched above") };
                let inner = carrier_inner(g);
                let name = FreshRef::parse(&inner)
                    .and_then(|r| match r {
                        FreshRef { group: Some(gp), start, end: FreshEnd::Single } => {
                            map.get(&(gp, start)).cloned()
                        }
                        _ => None,
                    })
                    .map(|n| {
                        let id = Ident::new(&n, v[i].span());
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
                let inner = g.stream().into_iter().collect();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::fresh::FreshCtx;

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
}
