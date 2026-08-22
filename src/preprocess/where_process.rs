//! Bare `where` new-syntax preprocessing.
//!
//! [`where_process`] scans the token stream after directive preprocessing and
//! before DSL parsing for the bare `where predicates {code block}` form:
//! collects predicates up to a boundary (a `{...}` code block, an ident
//! `where`, an `impl{...}` shape template, a depth-0 `;`, or the end of the
//! token stream) and rewrites it into the legacy `where{predicates}` suffix.
//! A bare `where` with **no** trailing code block is legal: the predicates
//! ride into a `where{...}` suffix with an empty body (`where A: Clone` ≡
//! `where A: Clone {}`). Shared by all three entries (`#[batch_impl]` /
//! `#[batch_impl_only]` / `batch_trait!`) and the impl entry; the parse layer
//! need not know about the new syntax.
//!
//! **Boundary rule**: the scan operates on the top-level token list only —
//! `angle_collect` has already paired `<...>` into opaque groups, and
//! proc-macro2 aggregates balanced `(...)`/`[...]` into single Group tokens,
//! so nested code blocks like `Fn({code})` are never mistaken for the body
//! boundary.
//!
//! Stop conditions: a depth-0 `;` ends the predicate region (the `;` stays
//! in the stream — it is the impl entry spec separator / the `batch_trait!`
//! segment boundary), and the end of the token stream ends it too (the
//! predicates become a body-less `where{...}` suffix).

use proc_macro2::{Group, TokenStream, TokenTree};

use crate::util::compile_error_str;
use crate::util::{bracket_is_passthrough, is_impl_template, is_punct};

pub(crate) fn where_process(tokens: &[TokenTree]) -> Result<Vec<TokenTree>, TokenStream> {
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
            let Some((where_body, rest_index)) = scan_body_boundary(&tokens[i + 1..]) else {
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
            let vt = where_process(&v)?;
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
/// template (`impl{...}` is an attachment, never a
/// predicate), or a depth-0 `;` (impl entry spec separator / `batch_trait!`
/// segment boundary, left in the stream). The end of the token stream is also
/// a boundary: the predicates ride into a body-less `where{...}` suffix
/// (bare `where A: Clone` ≡ `where A: Clone {}`).
fn scan_body_boundary(tokens: &[TokenTree]) -> Option<(TokenTree, usize)> {
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
    // End of the stream: the predicate region ends with the spec. A bare
    // `where` needs **some** predicates (an empty `where` with no body is a
    // typo); a non-empty predicate list becomes a body-less `where{...}`
    // suffix.
    if !result.is_empty() {
        return (Group::new(delimiter![{}], result.into_iter().collect()).into(), j).into();
    }
    None
}

fn is_macro_body(tokens: &[TokenTree], index: usize) -> bool {
    index >= 2
        && is_punct(&tokens[index - 1], '!')
        && matches!(&tokens[index - 2], TokenTree::Ident(_))
}
