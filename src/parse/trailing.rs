//! Trailing `{body}` / `where{...}` / `impl{...}` split and wrapper attachment.

use crate::ast::*;
use crate::parse::parse_primitive;
use proc_macro2::{Ident, TokenStream, TokenTree};

pub(crate) struct TrailingBody<'a> {
    /// Remaining tokens after stripping the trailing code block
    pub(crate) tokens: &'a [TokenTree],
    /// The stripped body; `None` means there is no trailing code block
    pub(crate) body: Option<TokenStream>,
    /// `true` when the body is a `where{...}` predicate suffix
    pub(crate) is_where: bool,
    /// `true` when the body is an `impl{...}` Self-part shape template (Ext 2)
    pub(crate) is_impl: bool,
}

/// Split off a trailing `{...}` code block (`macro!{...}` excluded; `where{...}`
/// is a predicate suffix, `impl{...}` is a Self-part shape template). The three
/// attachment kinds are peeled in any order by the caller's loop.
pub(crate) fn split_trailing_body(tokens: &[TokenTree]) -> TrailingBody<'_> {
    match tokens.last() {
        Some(TokenTree::Group(group)) if group.delimiter() == delimiter![{}] => {
            // macro!{...} is not a trailing code block; exclude it
            if tokens.len() >= 2
                && let TokenTree::Punct(p) = &tokens[tokens.len() - 2]
                && p.as_char() == '!'
            {
                return TrailingBody { tokens, body: None, is_where: false, is_impl: false };
            }
            // `where{...}` predicate suffix
            if tokens.len() >= 2
                && let TokenTree::Ident(i) = &tokens[tokens.len() - 2]
                && *i == "where"
            {
                return TrailingBody {
                    tokens: &tokens[..tokens.len() - 2],
                    body: group.stream().into(),
                    is_where: true,
                    is_impl: false,
                };
            }
            // `impl{...}` Self-part shape template (Ext 2) — an `impl` ident
            // directly followed by a brace group at the spec level. Bodies
            // never contain a top-level `impl {` (the whole body is one group
            // and is peeled as the ordinary code block, not entered).
            if tokens.len() >= 2
                && let TokenTree::Ident(i) = &tokens[tokens.len() - 2]
                && *i == "impl"
            {
                return TrailingBody {
                    tokens: &tokens[..tokens.len() - 2],
                    body: group.stream().into(),
                    is_where: false,
                    is_impl: true,
                };
            }
            TrailingBody {
                tokens: &tokens[..tokens.len() - 1],
                body: group.stream().into(),
                is_where: false,
                is_impl: false,
            }
        }
        _ => TrailingBody { tokens, body: None, is_where: false, is_impl: false },
    }
}

/// Wrapper kind (`WithAttr`/`WithPrefix` half-applied, inner `None`): empty
/// rest keeps the half-applied node, otherwise apply to the parsed remainder.
/// The remainder is operator-free by construction (the caller's segment cut
/// stops at `.`/`-`/`,`), so the Prim level suffices. `depth` threads the
/// chained-segment guard through the parse_primitive recursion.
pub(crate) fn attach_wrapper(
    kind: TyKind, rest: &[TokenTree], trait_name: Option<&Ident>, depth: usize,
) -> Ty {
    let base = Ty { span: proc_macro2::Span::call_site(), kind };
    if rest.is_empty() { base } else { base.apply(parse_primitive(rest, trait_name, depth + 1)) }
}
