//! Bare `where` new-syntax preprocessing.
//!
//! [`where_process`] scans the token stream after directive preprocessing and
//! before DSL parsing for the bare `where predicates {code block}` form:
//! collects predicates up to the first top-level `{...}` code block
//! (excluding `ident!{...}` macro-call bodies), rewrites it into the legacy
//! `where{predicates}` suffix; a missing code
//! block reports `compile_error!`. Shared by all three entries
//! (`#[batch_impl]` / `#[batch_impl_only]` / `batch_trait!`); the parse layer
//! need not know about the new syntax.
//!
//! **Boundary rule**: the scan operates on the top-level token list only —
//! `angle_collect` has already paired `<...>` into opaque groups, and
//! proc-macro2 aggregates balanced `(...)`/`[...]` into single Group tokens,
//! so nested code blocks like `Fn({code})` are never mistaken for the body
//! boundary.

use proc_macro2::{Group, TokenStream, TokenTree};

use crate::util::compile_error_str;
use crate::util::{Cursor, bracket_is_passthrough};

pub(crate) fn where_process(
    cursor: &mut Cursor,
) -> Result<Vec<TokenTree>, TokenStream> {
    let tokens = cursor.take_rest();
    let mut result = vec![];
    let mut i = 0;
    while i < tokens.len() {
        // Bare `where`: a directly following {group} is the legacy
        // `where{...}`, skipped as-is; otherwise rewrite into where{predicates}
        if let TokenTree::Ident(ident) = &tokens[i]
            && ident == "where"
            && i + 1 < tokens.len()
            && !matches!(&tokens[i+1],TokenTree::Group(g)
                if g.delimiter() == delimiter![{}])
        {
            let Some((where_body, rest_index)) = scan_body_boundary(&tokens[i + 1..])
            else {
                return Err(compile_error_str(
                    "batch-impl: `where` predicates are missing a code block {...}",
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
            let vt = where_process(&mut Cursor::new(&v))?;
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
/// `ident!{...}` macro bodies) or an ident `where`.
fn scan_body_boundary(tokens: &[TokenTree]) -> Option<(TokenTree, usize)> {
    let mut j = 0;
    let mut result = vec![];
    while j < tokens.len() {
        match &tokens[j] {
            TokenTree::Group(g)
                if g.delimiter() == delimiter![{}] && !is_macro_body(tokens, j) =>
            {
                return (
                    Group::new(delimiter![{}], result.into_iter().cloned().collect())
                        .into(),
                    j,
                )
                    .into();
            }
            TokenTree::Ident(w) if w == "where" => {
                return (
                    Group::new(delimiter![{}], result.into_iter().cloned().collect())
                        .into(),
                    j,
                )
                    .into();
            }
            _ => result.push(&tokens[j]),
        }
        j += 1;
    }
    None
}

fn is_macro_body(tokens: &[TokenTree], index: usize) -> bool {
    index >= 2
        && matches!(&tokens[index - 1], TokenTree::Punct(p) if p.as_char() == '!')
        && matches!(&tokens[index - 2], TokenTree::Ident(_))
}
