//! Scan rendered Rust type tokens without entering expression or macro domains.

use proc_macro2::{Delimiter, Span, TokenStream, TokenTree};

use super::{TokenDomain, group_at, is_punct_at};

/// Find a token pattern in type positions. Braced const arguments, array
/// lengths, literals and opaque macro/attribute inputs are not type syntax.
/// An explicit work stack avoids adding recursion to deeply nested types.
pub(crate) fn find_type_token(
    tokens: &TokenStream, check: impl Fn(&[TokenTree], usize) -> Option<Span>,
) -> Option<Span> {
    let mut stack = vec![(tokens.clone().into_iter().collect::<Vec<_>>(), 0usize)];
    while let Some((trees, index)) = stack.last_mut() {
        let i = *index;
        let Some(tree) = trees.get(i) else {
            stack.pop();
            continue;
        };
        *index += 1;
        // The leading `#` is part of an attribute, not a DSL marker. Skip
        // the entire attribute before checking tokens, including inner attrs.
        if is_punct_at(trees, i, '#') {
            let group = i + 1 + usize::from(is_punct_at(trees, i + 1, '!'));
            if group_at(trees, group, Delimiter::Bracket).is_some() {
                *index = group + 1;
                continue;
            }
        }
        if let Some(span) = check(trees, i) {
            return Some(span);
        }
        if let TokenTree::Group(group) = tree
            && !TokenDomain::Type.skips_group(trees, i)
        {
            let mut inner = group.stream().into_iter().collect::<Vec<_>>();
            if group.delimiter() == Delimiter::Bracket
                && let Some(end) = inner
                    .iter()
                    .position(|t| matches!(t, TokenTree::Punct(p) if p.as_char() == ';'))
            {
                inner.truncate(end);
            }
            stack.push((inner, 0));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    fn star(tokens: &TokenStream) -> Option<Span> {
        find_type_token(tokens, |trees, i| match trees.get(i) {
            Some(TokenTree::Punct(p)) if p.as_char() == '*' => Some(p.span()),
            _ => None,
        })
    }

    #[test]
    fn excludes_expression_and_opaque_domains() {
        for tokens in [
            quote!(Array<{ 2 * 3 }>),
            quote!([u8; 2 * 3]),
            quote!(M<ty!(*anything), ty![*anything], ty! { *anything }>),
            quote!(M<{ "*".len() }>),
        ] {
            assert!(star(&tokens).is_none(), "{tokens}");
        }
    }

    #[test]
    fn visits_types_inside_each_type_container() {
        for tokens in [quote!(M<*T>), quote!([*T; 2]), quote!(fn(u8) -> (*T,))] {
            assert!(star(&tokens).is_some(), "{tokens}");
        }
    }

    #[test]
    fn attributes_are_atomic_but_stray_hashes_remain_visible() {
        // Strings keep rustfmt from normalizing away fn-parameter attributes
        // while formatting the otherwise Rust-shaped contents of quote!.
        for (source, expected) in [
            ("fn(#[allow(unused)] u8)", false),
            ("fn(#[cfg_attr(any(), allow(unused))] fn(#[allow(unused)] u8))", false),
            ("fn(#![opaque(#dsl)] u8)", false),
            ("fn(#[allow(unused)] #dsl)", true),
            ("fn(#(not_an_attribute) u8)", true),
            ("fn(#!(not_an_attribute) u8)", true),
        ] {
            let tokens: TokenStream = source.parse().unwrap();
            let found = find_type_token(&tokens, |trees, i| match trees.get(i) {
                Some(TokenTree::Punct(p)) if p.as_char() == '#' => Some(p.span()),
                _ => None,
            });
            assert_eq!(found.is_some(), expected, "{tokens}");
        }
    }
}
