//! Declaration-only scope carriers preserve empty generator groups across re-entry.

use proc_macro2::{Delimiter, Group, Literal, Span, TokenStream, TokenTree};
use quote::quote;

use super::fresh_protocol::carrier_group_at;

/// This is scope metadata, not a fresh-position reference or a Rust parameter.
pub(crate) fn fresh_group_tokens(group: usize) -> TokenStream {
    let group = Literal::usize_unsuffixed(group);
    let mut payload = Group::new(Delimiter::Brace, quote!(group #group));
    payload.set_span(Span::call_site());
    quote!(@ #payload)
}

/// Recognized only in a standalone declaration slot, never as a type argument.
pub(crate) fn decl_fresh_group(tokens: &TokenStream) -> Option<usize> {
    let tokens = tokens.clone().into_iter().collect::<Vec<_>>();
    if tokens.len() != 2 {
        return None;
    }
    let payload = carrier_group_at(&tokens, 0)?.stream().into_iter().collect::<Vec<_>>();
    match payload.as_slice() {
        [TokenTree::Ident(tag), TokenTree::Literal(group)] if tag == "group" => {
            group.to_string().parse().ok()
        }
        _ => None,
    }
}
