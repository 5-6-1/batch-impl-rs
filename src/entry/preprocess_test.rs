//! `batch_preprocess_test!` — the reference implementation of the open-extension
//! protocol (also used as the test consumer). Split from lib.rs so the crate-root
//! entry points stay thin (proc-macro functions must live at the crate root,
//! their implementations do not).

use proc_macro2::{TokenStream, TokenTree};
use quote::quote;

use crate::preprocess::{
    angle_collect, build_from_item, get_trait_item, parse_names_from_tokens, render_angles,
};
use crate::util::compile_error_str;

/// `batch_preprocess_test!` implementation — the lib.rs entry point hands the
/// raw macro input here (see its doc for the protocol).
pub(crate) fn preprocess_test(input: TokenStream) -> Result<TokenStream, TokenStream> {
    let tokens = input.into_iter().collect::<Vec<_>>();
    let tokens = angle_collect(&tokens)?;
    // Shape: `{spec}(method name list){body} trait ...` (top-level form —
    // the first Brace group is the spec body; the macro emits a full impl
    // for it) or the legacy `(method name list){body} trait ...` (in-impl
    // form — emits associated fn definitions for the enclosing impl).
    let spec = match tokens.first() {
        Some(TokenTree::Group(g))
            if g.delimiter() == delimiter![{}]
                && matches!(
                    tokens.get(1),
                    Some(TokenTree::Group(p)) if p.delimiter() == delimiter![()]
                ) =>
        {
            Some(g.stream())
        }
        _ => None,
    };
    let idx = if spec.is_some() { 1 } else { 0 };
    let Some(TokenTree::Group(names_group)) = tokens.get(idx) else {
        return Err(compile_error_str(
            "batch-impl: batch_preprocess_test expects `(method name list){body} trait ...`",
            tokens.first().map(|t| t.span()).unwrap_or_else(proc_macro2::Span::call_site),
        ));
    };
    if names_group.delimiter() != delimiter![()] {
        return Err(compile_error_str(
            "batch-impl: batch_preprocess_test expects `(method name list){body} trait ...`",
            tokens.first().map(|t| t.span()).unwrap_or_else(proc_macro2::Span::call_site),
        ));
    }
    let Some(TokenTree::Group(body_group)) = tokens.get(idx + 1) else {
        return Err(compile_error_str(
            "batch-impl: batch_preprocess_test expects `(method name list){body} trait ...`",
            tokens.get(1).map(|t| t.span()).unwrap_or_else(proc_macro2::Span::call_site),
        ));
    };
    if body_group.delimiter() != delimiter![{}] {
        return Err(compile_error_str(
            "batch-impl: batch_preprocess_test expects `(method name list){body} trait ...`",
            tokens.get(1).map(|t| t.span()).unwrap_or_else(proc_macro2::Span::call_site),
        ));
    }
    let trait_ts = tokens[idx + 2..].iter().cloned().collect();
    let trait_item = match syn::parse2(trait_ts) {
        Ok(t) => t,
        Err(_) => {
            return Err(compile_error_str(
                "batch-impl: batch_preprocess_test cannot parse the trait definition",
                proc_macro2::Span::call_site(),
            ));
        }
    };
    let names = parse_names_from_tokens(
        &names_group.stream().into_iter().collect::<Vec<_>>(),
        &trait_item,
    )?;
    let body = body_group.stream();
    let mut methods = TokenStream::new();
    for name in &names {
        let item = get_trait_item(&trait_item, name)?;
        methods.extend(build_from_item(item, &body));
    }
    Ok(match spec {
        // Top-level form: emit a full impl for the spec body (`{spec}` first
        // segment) — the batch_impl crate emits no impl in this mode.
        Some(spec_ts) => {
            let ident = &trait_item.ident;
            render_angles(quote!(impl #ident for #spec_ts { #methods }))
        }
        None => render_angles(methods),
    })
}
