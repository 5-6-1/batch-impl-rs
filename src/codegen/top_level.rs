//! Top-level macro injection (`{! ...}`): the open-extension product / manual
//! attach form is recognized on a `WithCode` chain, the spec body (target +
//! preceding blocks in chain order) is merged into one Brace group, and the
//! macro call is rewritten to `name!{ {spec}(args){body}trait }`.

use proc_macro2::{Group, TokenStream, TokenTree};
use quote::quote;

use crate::ast::*;
use crate::util::compile_error_str;

/// Detects the top-level macro form: a `WithCode` chain ending in a
/// `{! ...}` block (`!` as the block's first token — the open-extension
/// product or a user-written `T {! m!{...}}`). Returns the spec body tokens
/// (target type + preceding blocks in chain order, rendered) and the macro
/// call after the `!`. A `{!}` block must be the last block and there can
/// be at most one.
pub(crate) fn top_level_macro(ty: &Ty) -> Option<Result<(TokenStream, TokenStream), TokenStream>> {
    let mut body = vec![];
    let mut top: Option<TokenStream> = None;
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
    let mut out: Vec<TokenTree> = Vec::with_capacity(tokens.len() + 1);
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
            let spec_group = Group::new(proc_macro2::Delimiter::Brace, spec.clone());
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
