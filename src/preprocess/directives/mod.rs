//! The `#` directive system — `#fill` / `#delegate` / `#blanket` and the open
//! extension (`#name(args){body}` → top-level macro call).
//!
//! Files:
//! - [`dispatch`] — the dispatch table (`#name{body}` / `#cmd(args){body}`)
//!   and the `#fill` / `#delegate` / single-item expansions;
//! - [`name_list`] — directive argument name lists (`@all` markers,
//!   `-name` / `-[a, b]` subtraction);
//! - [`trait_items`] — trait item lookups (`#name` / `#fill` / `#delegate`
//!   resolve item signatures from the annotated trait) plus the `@all`-family
//!   marker specs;
//! - [`delegate_args`] — delegate argument forwarding patterns;
//! - [`delegate_template`] — local `receiver.#call` sites and hygienic
//!   argument forwarding inside complete method-body templates;
//! - [`blanket`] — `#blanket` expansion (wrapper matrix → delegation specs);
//! - [`blanket_wrappers`] — blanket wrapper parsing (`wrapper.T` forms).

mod blanket;
mod blanket_helpers;
#[cfg(test)]
mod blanket_helpers_boundary_tests;
mod blanket_wrappers;
mod delegate_args;
mod delegate_template;
mod dispatch;
mod name_list;
mod trait_items;

pub(crate) use blanket::expand_blanket;
pub(crate) use blanket_wrappers::*;
pub(crate) use delegate_args::*;
pub(crate) use dispatch::expand_directive;
pub(crate) use name_list::*;
pub(crate) use trait_items::*;

use proc_macro2::{Group, TokenStream, TokenTree};

use crate::util::compile_error_str;

/// Rejects `#name(...)` directives (only `#[...]` attributes pass through) —
/// used by the ItemImpl entry, which has no directive system. `@` is handled
/// earlier by `expand_consts` (built-in constants + `@trait`). Establishes
/// the same "no bare `#` left" invariant as [`expand_tokens`] (which expands
/// them) in visible DSL positions — hence both map `Paired → DirectivesResolved`
/// in the typestate pipeline. Ordinary braces and macro/attribute inputs are
/// preserved; explicit DSL attachments keep their normal traversal boundary.
pub(crate) fn reject_directives(tokens: &[TokenTree]) -> Result<Vec<TokenTree>, TokenStream> {
    let mut out = vec![];
    let mut i = 0;
    while let Some(cur) = tokens.get(i) {
        match cur {
            // `#` directives are banned; `#[...]` attributes pass through.
            TokenTree::Punct(p) if p.as_char() == '#' => {
                if let Some(g) = crate::util::group_at(tokens, i + 1, delimiter![[]]) {
                    out.push(cur.clone());
                    out.push(TokenTree::Group(g.clone()));
                    i += 2;
                } else {
                    return Err(compile_error_str(
                        "batch-impl: `#` directives are not supported on the ItemImpl entry \
                         (write the impl body directly)",
                        cur.span(),
                    ));
                }
            }
            TokenTree::Group(g) if !crate::util::TokenDomain::Type.skips_group(tokens, i) => {
                let inner = reject_directives(&g.stream().into_iter().collect::<Vec<_>>())?;
                let mut ng = Group::new(g.delimiter(), inner.into_iter().collect());
                ng.set_span(g.span());
                out.push(TokenTree::Group(ng));
                i += 1;
            }
            _ => {
                out.push(cur.clone());
                i += 1;
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejection_preserves_rust_blocks_macro_inputs_and_attributes() {
        for source in [
            "Arr<{ six!(#token) }>",
            "ty!(#token)",
            "ty![#token]",
            "ty!{#token}",
            "fn(#[opaque(#token)] u8)",
            "where { Arr<{ #token }>: Bound<ty!(#token)> }",
            "impl { Arr<{ #token }> }",
            "@{ #token }",
        ] {
            let source: TokenStream = source.parse().unwrap();
            let output = reject_directives(&source.clone().into_iter().collect::<Vec<_>>())
                .unwrap()
                .into_iter()
                .collect::<TokenStream>();
            assert_eq!(output.to_string(), source.to_string());
        }
    }

    #[test]
    fn rejection_still_reaches_directives_in_dsl_type_groups() {
        for source in
            ["#fill", "(u8, #fill)", "[u8, (#fill)]", "where { u8: #fill }", "impl { (#fill,) }"]
        {
            let tokens = source.parse::<TokenStream>().unwrap().into_iter().collect::<Vec<_>>();
            let error = reject_directives(&tokens).unwrap_err().to_string();
            assert!(error.contains("directives are not supported"), "{source}: {error}");
        }
    }
}
