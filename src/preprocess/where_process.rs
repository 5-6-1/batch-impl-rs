//! Bare `where` new-syntax preprocessing.
//!
//! [`where_process`] scans the token stream after directive preprocessing and
//! before DSL parsing for the bare `where predicates {code block}` form:
//! collects predicates up to the first top-level `{...}` code block
//! (excluding `ident!{...}` macro-call bodies) and rewrites it into the legacy
//! `where{predicates}` suffix; a missing code
//! block reports `compile_error!`. Shared by all three entries
//! (`#[batch_impl]` / `#[batch_impl_only]` / `batch_trait!`) and the Ext 1
//! ItemImpl entry (`allow_end`); the parse layer need not know about the new
//! syntax.
//!
//! **Boundary rule**: the scan operates on the top-level token list only —
//! `angle_collect` has already paired `<...>` into opaque groups, and
//! proc-macro2 aggregates balanced `(...)`/`[...]` into single Group tokens,
//! so nested code blocks like `Fn({code})` are never mistaken for the body
//! boundary.
//!
//! Stop conditions (0.8.0 Ext 1): a depth-0 `;` ends the predicate region
//! (the `;` stays in the stream — it is the Ext 1 spec separator / the
//! `batch_trait!` segment boundary), and with `allow_end` the end of the
//! token stream ends it too (the Ext 1 ItemImpl attr has no body after the
//! predicates).

use proc_macro2::{Group, TokenStream, TokenTree};

use crate::util::compile_error_str;
use crate::util::{bracket_is_passthrough, is_impl_template, is_punct};

pub(crate) fn where_process(
    tokens: &[TokenTree], allow_end: bool,
) -> Result<Vec<TokenTree>, TokenStream> {
    let mut result = vec![];
    let mut i = 0;
    while i < tokens.len() {
        // Bare `where`: a directly following {group} is the legacy
        // `where{...}`, skipped as-is; otherwise rewrite into where{predicates}.
        // (`where` is a Rust keyword — an Ident `where` can only be the DSL
        // form, so a missing body always errors here, not at the parse layer.)
        if let TokenTree::Ident(ident) = &tokens[i]
            && ident == "where"
            && !matches!(tokens.get(i + 1), Some(TokenTree::Group(g))
                if g.delimiter() == delimiter![{}])
        {
            let Some((where_body, rest_index)) = scan_body_boundary(&tokens[i + 1..], allow_end)
            else {
                return Err(compile_error_str(
                    "batch-impl: `where` predicates are missing a code block {...}",
                    tokens[i].span(),
                ));
            };
            result.push(ident.clone().into());
            result.push(where_body);
            i += 1 + rest_index;
        } else if let TokenTree::Group(g) = &tokens[i]
            && g.delimiter() == delimiter!([])
            // `ident![...]` macro bodies and `#[...]` attributes passthrough,
            // no recursion
            && !bracket_is_passthrough(tokens, i)
        {
            let v = g.stream().into_iter().collect::<Vec<_>>();
            let vt = where_process(&v, allow_end)?;
            result.push(Group::new(delimiter![[]], vt.into_iter().collect()).into());
            i += 1
        } else {
            result.push(tokens[i].clone());
            i += 1;
        };
    }
    Ok(result)
}

/// The predicate-region boundary = the first `{...}` group (excluding
/// `ident!{...}` macro bodies), an ident `where`, an `impl{...}` shape
/// template (Ext 2 — `impl{...}` is a Self-part attachment, never a
/// predicate), or a depth-0 `;` (Ext 1 spec separator / `batch_trait!`
/// segment boundary, left in the stream). With `allow_end` the end of the
/// token stream is also a boundary (Ext 1 ItemImpl attr: no body follows).
fn scan_body_boundary(tokens: &[TokenTree], allow_end: bool) -> Option<(TokenTree, usize)> {
    let mut j = 0;
    let mut result: Vec<TokenTree> = vec![];
    while j < tokens.len() {
        match &tokens[j] {
            TokenTree::Group(g) if g.delimiter() == delimiter![{}] && !is_macro_body(tokens, j) => {
                return (Group::new(delimiter![{}], result.into_iter().collect()).into(), j).into();
            }
            TokenTree::Ident(w) if w == "where" => {
                return (Group::new(delimiter![{}], result.into_iter().collect()).into(), j).into();
            }
            // `impl{...}` attachment boundary: an `impl` ident directly
            // followed by a Brace group ends the predicate region (the
            // attachment is peeled by the parse layer). `impl Trait` /
            // `impl Fn(...)` in a predicate are followed by an Ident/group,
            // not a Brace, so they stay part of the predicate. The
            // discrimination is centralized in `util::is_impl_template`.
            TokenTree::Ident(_) if is_impl_template(tokens, j) => {
                return (Group::new(delimiter![{}], result.into_iter().collect()).into(), j).into();
            }
            // `;` ends the predicate region; the `;` itself stays in the
            // stream (spec separator / segment boundary).
            TokenTree::Punct(p) if p.as_char() == ';' => {
                return (Group::new(delimiter![{}], result.into_iter().collect()).into(), j).into();
            }
            _ => result.push(tokens[j].clone()),
        }
        j += 1;
    }
    // End of the stream: legal only when the caller permits it (Ext 1).
    if allow_end {
        return (Group::new(delimiter![{}], result.into_iter().collect()).into(), j).into();
    }
    None
}

fn is_macro_body(tokens: &[TokenTree], index: usize) -> bool {
    index >= 2
        && is_punct(&tokens[index - 1], '!')
        && matches!(&tokens[index - 2], TokenTree::Ident(_))
}
