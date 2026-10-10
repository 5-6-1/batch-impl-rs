//! A conservative, invocation-local duplicate check on finalized Rust impls.
//!
//! This is not coherence checking: unresolved syntax, attributes and opaque
//! macros stay with rustc. Only top-level impls are compared, never user bodies
//! or items hidden inside an extension's macro input.

use std::collections::HashMap;

use proc_macro2::{Span, TokenStream, TokenTree};
use quote::{ToTokens, quote};

use crate::entry::duplicate_key;

/// Check the complete output of one entry, after angle restoration and fresh
/// naming. Return the original stream unchanged, or only duplicate diagnostics.
pub(super) fn check(output: TokenStream) -> Result<TokenStream, TokenStream> {
    let Some(heads) = super::duplicate_heads::heads(output.clone()) else {
        // Unsupported Rust syntax is still rustc's responsibility.
        return Ok(output);
    };
    let mut seen: HashMap<String, (usize, Span)> = HashMap::new();
    let mut errors = TokenStream::new();
    let mut ordinal = 0;
    for mut item in heads {
        ordinal += 1;
        let Some(key) = duplicate_key::key(&item) else { continue };
        let span = target_span(item.self_ty.to_token_stream());
        if let Some((first, first_span)) = seen.get(&key) {
            item.items.clear();
            let head = quote!(#item).to_string();
            let message = format!(
                "batch-impl: duplicate generated trait impl #{ordinal}: `{head}`; \
                 repeats impl #{first} after generic renaming; remove the repeated target \
                 or use disjoint specs"
            );
            // The diagnostic's literal and delimiter need the source span as
            // well as its macro name. syn emits a hygienic absolute path and
            // braces, so this remains a valid item without a trailing `;`.
            errors.extend(syn::Error::new(span, message).into_compile_error());
            errors.extend(
                syn::Error::new(
                    *first_span,
                    format!("batch-impl: first occurrence of this generated trait impl (#{first})"),
                )
                .into_compile_error(),
            );
        } else {
            seen.insert(key, (ordinal, span));
        }
    }
    if errors.is_empty() { Ok(output) } else { Err(errors) }
}

/// Generated delimiter groups can have call-site spans even when their type
/// tokens retain the user's location. Prefer the first concrete type token.
fn target_span(tokens: TokenStream) -> Span {
    let fallback = tokens.clone().into_iter().next().map_or_else(Span::call_site, |t| t.span());
    let mut pending = vec![tokens.into_iter()];
    while let Some(tokens) = pending.last_mut() {
        match tokens.next() {
            Some(TokenTree::Group(group)) => {
                pending.push(group.stream().into_iter());
            }
            // Prefix punctuation can be generated even when its operand's
            // identifier retains the location of the original target.
            Some(TokenTree::Punct(punct)) if punct.as_char() == '\'' => {
                // A rendered lifetime can recreate its identifier as well.
                // These are parsed type tokens, so the next token is its name.
                tokens.next();
            }
            Some(TokenTree::Punct(_)) => {}
            Some(TokenTree::Ident(id))
                if matches!(
                    id.to_string().as_str(),
                    "const" | "mut" | "unsafe" | "fn" | "extern" | "dyn" | "for"
                ) => {}
            Some(token) => return token.span(),
            None => {
                pending.pop();
            }
        }
    }
    fallback
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_failure_withholds_all_generated_items() {
        let error = check(quote! {
            trait Rule { fn value() -> u8; }
            impl Rule for u16 { fn value() -> u8 { 1 } }
            impl Rule for u8 { fn value() -> u8 { 2 } }
            impl Rule for u8 { fn value() -> u8 { 3 } }
        })
        .unwrap_err();
        let file = syn::parse2::<syn::File>(error.clone()).unwrap();
        assert!(file.items.iter().all(|item| matches!(item, syn::Item::Macro(_))));
        let message = error.to_string();
        assert!(message.contains("impl #3"), "{message}");
        assert!(message.contains("repeats impl #2"), "{message}");
        assert!(!message.contains("fn value"), "{message}");
    }

    #[test]
    fn duplicate_diagnostic_literals_keep_both_target_spans() {
        for (head, marker, width) in [
            ("impl Rule for u8 {}", "u8", 2),
            ("impl<'a, const N: usize> Rule for (&'a [u8; N],) {}", "u8", 2),
            ("impl Rule for *const u8 {}", "u8", 2),
            ("impl Rule for unsafe fn() -> u8 {}", "u8", 2),
            ("impl Rule for <u8 as Tr>::Item {}", "u8", 2),
            ("impl Rule for ((), u8) {}", "u8", 2),
        ] {
            let column = head.find(marker).unwrap();
            let error = check(format!("{head}\n{head}").parse().unwrap()).unwrap_err();
            let file = syn::parse2::<syn::File>(error).unwrap();
            assert_eq!(file.items.len(), 2);
            for (item, line) in file.items.into_iter().zip([2, 1]) {
                let syn::Item::Macro(item) = item else { panic!("expected diagnostic") };
                assert!(item.mac.path.leading_colon.is_some());
                assert_eq!(item.mac.path.to_token_stream().to_string(), ":: core :: compile_error");
                assert!(matches!(item.mac.delimiter, syn::MacroDelimiter::Brace(_)));
                let literal = syn::parse2::<syn::LitStr>(item.mac.tokens).unwrap();
                assert_eq!(literal.span().start().line, line, "{head}");
                assert_eq!(literal.span().start().column, column, "{head}");
                assert_eq!(literal.span().end().column, column + width, "{head}");
            }
        }
    }

    #[test]
    fn opaque_nested_items_are_not_compared() {
        let output = quote! {
            extension! { impl Rule for u8 {} impl Rule for u8 {} }
            mod scoped { impl Rule for u8 {} }
            impl Rule for u8 { fn value() { opaque!(impl Rule for u8 {}); } }
        };
        assert_eq!(check(output.clone()).unwrap().to_string(), output.to_string());
    }

    #[test]
    fn unsupported_rust_is_left_to_the_compiler() {
        let output = quote!(impl Rule for u8 { not rust });
        assert_eq!(check(output.clone()).unwrap().to_string(), output.to_string());
    }

    #[test]
    fn inner_cfg_does_not_become_an_unconditional_duplicate() {
        let output = quote! {
            impl Rule for u8 { #![cfg(any())] }
            impl Rule for u8 { #![cfg(any())] }
        };
        assert_eq!(check(output.clone()).unwrap().to_string(), output.to_string());
    }
}
