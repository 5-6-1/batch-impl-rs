//! Read finalized impl headers without parsing opaque Rust bodies.
//!
//! A brace is an item boundary only when replacing that brace group with an
//! empty body makes a complete item. Earlier const-generic braces remain in
//! the header, so `Trait<{ N }> for Target` is not cut at the const expression.

use proc_macro2::{Delimiter, Group, TokenStream, TokenTree};

/// Unknown items or expensive headers disable this optional diagnostic for
/// the invocation. Neither opaque bodies nor macro inputs are expanded.
pub(super) fn heads(output: TokenStream) -> Option<Vec<syn::ItemImpl>> {
    let mut prefix = TokenStream::new();
    let mut heads = vec![];
    let mut macro_semicolon = false;
    for token in output {
        if macro_semicolon {
            if !matches!(&token, TokenTree::Punct(p) if p.as_char() == ';') {
                return None;
            }
            macro_semicolon = false;
            continue;
        }
        if let TokenTree::Group(body) = &token
            && body.delimiter() != Delimiter::None
        {
            // A macro's input is opaque even when its delimiter is `()` or
            // `[]`. Test only its path and an empty input group.
            let mut empty = Group::new(body.delimiter(), TokenStream::new());
            empty.set_span(body.span());
            let mut candidate = prefix.clone();
            candidate.extend([TokenTree::Group(empty)]);
            if body.delimiter() != Delimiter::Brace {
                candidate.extend(quote::quote!(;));
            }
            if !small_header(candidate.clone()) {
                return None;
            }
            if syn::parse2::<syn::ItemMacro>(candidate).is_ok() {
                macro_semicolon = body.delimiter() != Delimiter::Brace;
                prefix = TokenStream::new();
                continue;
            }
        }
        if let TokenTree::Group(body) = &token
            && body.delimiter() == Delimiter::Brace
        {
            let mut empty = Group::new(Delimiter::Brace, TokenStream::new());
            empty.set_span(body.span());
            let mut candidate = prefix.clone();
            candidate.extend([TokenTree::Group(empty)]);
            if !small_header(candidate.clone()) {
                return None;
            }
            if let Ok(item) = syn::parse2::<syn::Item>(candidate) {
                match item {
                    syn::Item::Impl(item) => {
                        // Inner attributes belong to the impl, not its first
                        // member. Discarding a `#![cfg(...)]` here could make a
                        // disabled impl look unconditionally duplicated.
                        let mut first = body.stream().into_iter();
                        if matches!(first.next(), Some(TokenTree::Punct(p)) if p.as_char() == '#')
                            && matches!(first.next(), Some(TokenTree::Punct(p)) if p.as_char() == '!')
                        {
                            return None;
                        }
                        // A transparent group can hide a leading inner attr.
                        if matches!(body.stream().into_iter().next(), Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::None)
                        {
                            return None;
                        }
                        heads.push(item);
                    }
                    syn::Item::Trait(_) | syn::Item::Macro(_) | syn::Item::Mod(_) => {}
                    _ => return None,
                }
                prefix = TokenStream::new();
                continue;
            }
        }
        if matches!(&token, TokenTree::Punct(p) if p.as_char() == ';') {
            if prefix.is_empty() {
                continue;
            }
            prefix.extend([token]);
            if !small_header(prefix.clone())
                || !matches!(syn::parse2::<syn::Item>(prefix.clone()), Ok(syn::Item::Macro(_)))
            {
                return None;
            }
            prefix = TokenStream::new();
        } else {
            prefix.extend([token]);
        }
    }
    (prefix.is_empty() && !macro_semicolon).then_some(heads)
}

/// Bound syn's extra parsing work conservatively. Counting every group and
/// type-nesting token also covers flat angle/reference chains, which a group
/// depth check alone misses. Bodies never enter this iterative inspection.
fn small_header(tokens: TokenStream) -> bool {
    let mut pending = vec![tokens];
    let mut tokens = 0usize;
    let mut nesting_tokens = 0usize;
    while let Some(stream) = pending.pop() {
        for token in stream {
            tokens += 1;
            if tokens > 128 {
                return false;
            }
            match token {
                TokenTree::Group(group) => {
                    nesting_tokens += 1;
                    pending.push(group.stream());
                }
                TokenTree::Punct(p) if matches!(p.as_char(), '<' | '&' | '*') => {
                    nesting_tokens += 1;
                }
                _ => {}
            }
            if nesting_tokens > 16 {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::{ToTokens, quote};

    #[test]
    fn const_braces_are_part_of_the_header_not_the_body() {
        let output = quote! {
            impl Rule<{ 1 }> for Target where Target: Other<{ 2 }> {
                opaque body that is not Rust
            }
        };
        let found = heads(output).unwrap();
        assert_eq!(found.len(), 1);
        let item = found.first().unwrap();
        assert!(item.items.is_empty());
        let header = item.to_token_stream().to_string();
        assert!(header.contains("Rule < { 1 } >"), "{header}");
        assert!(header.contains("Other < { 2 } >"), "{header}");
        assert!(!header.contains("opaque"), "{header}");
    }

    #[test]
    fn trait_module_and_macro_bodies_remain_opaque() {
        let output = quote! {
            pub trait Rule { arbitrary tokens }
            extension! { impl Rule for u8 {} }
            extension!(arbitrary tokens);
            mod nested { impl Rule for u8 {} }
            impl Rule for u8 { arbitrary tokens }
        };
        assert_eq!(heads(output).unwrap().len(), 1);
    }

    #[test]
    fn deeply_nested_body_is_not_given_to_syn() {
        let mut body = quote!(opaque);
        for _ in 0..256 {
            body = TokenTree::Group(Group::new(Delimiter::Brace, body)).into();
        }
        let mut output = quote!(impl Rule for u8);
        output.extend([TokenTree::Group(Group::new(Delimiter::Brace, body))]);
        assert_eq!(heads(output).unwrap().len(), 1);
    }

    #[test]
    fn deeply_nested_macro_input_is_not_given_to_syn() {
        let mut body = quote!(opaque);
        for _ in 0..256 {
            body = TokenTree::Group(Group::new(Delimiter::Parenthesis, body)).into();
        }
        let mut output = quote!(extension!);
        output.extend([TokenTree::Group(Group::new(Delimiter::Parenthesis, body))]);
        output.extend(quote!(; impl Rule for u8 {}));
        assert_eq!(heads(output).unwrap().len(), 1);
    }

    #[test]
    fn inner_attributes_and_unknown_items_are_left_to_rust() {
        for output in [
            quote!(impl Rule for u8 { #![cfg(any())] fn f() {} }),
            quote!(impl Rule for u8 { #![allow(dead_code)] }),
            quote!(const VALUE: usize = { 1 }; impl Rule for u8 {}),
        ] {
            assert!(heads(output).is_none());
        }
    }

    #[test]
    fn deep_angle_and_reference_headers_are_skipped_before_parsing() {
        for target in
            [format!("{}u8{}", "Box<".repeat(64), ">".repeat(64)), format!("{}u8", "&".repeat(64))]
        {
            let output = format!("impl Rule for {target} {{}}").parse().unwrap();
            assert!(heads(output).is_none());
        }
    }
}
