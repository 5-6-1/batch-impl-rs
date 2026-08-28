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
//! - [`blanket`] — `#blanket` expansion (wrapper matrix → delegation specs);
//! - [`blanket_wrappers`] — blanket wrapper parsing (`wrapper.T` forms).

mod blanket;
mod blanket_helpers;
mod blanket_wrappers;
mod delegate_args;
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
/// them) — hence both map `Paired → DirectivesResolved` in the typestate
/// pipeline.
pub(crate) fn reject_directives(tokens: &[TokenTree]) -> Result<Vec<TokenTree>, TokenStream> {
    let mut out = vec![];
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            // `#` directives are banned; `#[...]` attributes pass through.
            TokenTree::Punct(p) if p.as_char() == '#' => {
                if matches!(tokens.get(i + 1), Some(TokenTree::Group(g))
                    if g.delimiter() == delimiter![[]])
                {
                    out.push(tokens[i].clone());
                    out.push(tokens[i + 1].clone());
                    i += 2;
                } else {
                    return Err(compile_error_str(
                        "batch-impl: `#` directives are not supported on the ItemImpl entry \
                         (write the impl body directly)",
                        tokens[i].span(),
                    ));
                }
            }
            TokenTree::Group(g) => {
                let inner = reject_directives(&g.stream().into_iter().collect::<Vec<_>>())?;
                let mut ng = Group::new(g.delimiter(), inner.into_iter().collect());
                ng.set_span(g.span());
                out.push(TokenTree::Group(ng));
                i += 1;
            }
            _ => {
                out.push(tokens[i].clone());
                i += 1;
            }
        }
    }
    Ok(out)
}
