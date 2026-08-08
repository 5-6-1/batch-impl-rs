//! Trailing `{body}` / `where{...}` split and wrapper attachment.

use crate::ast::*;
use crate::parse::parse_primitive;
use crate::parse::split_at_depth0;
use proc_macro2::{Delimiter, Ident, TokenStream, TokenTree};

pub(crate) struct TrailingBody<'a> {
    /// Remaining tokens after stripping the trailing code block
    pub(crate) tokens: &'a [TokenTree],
    /// The stripped body; `None` means there is no trailing code block
    pub(crate) body: Option<TokenStream>,
    /// `true` when the body is a `where{...}` predicate suffix
    pub(crate) is_where: bool,
}

/// Split off a trailing `{...}` code block (`macro!{...}` excluded; `where{...}` is a predicate)
pub(crate) fn split_trailing_body(tokens: &[TokenTree]) -> TrailingBody<'_> {
    match tokens.last() {
        Some(TokenTree::Group(group)) if group.delimiter() == delimiter![{}] => {
            // macro!{...} is not a trailing code block; exclude it
            if tokens.len() >= 2
                && let TokenTree::Punct(p) = &tokens[tokens.len() - 2]
                && p.as_char() == '!'
            {
                return TrailingBody { tokens, body: None, is_where: false };
            }
            if tokens.len() >= 2
                && let TokenTree::Ident(i) = &tokens[tokens.len() - 2]
                && *i == "where"
            {
                return TrailingBody {
                    tokens: &tokens[..tokens.len() - 2],
                    body: group.stream().into(),
                    is_where: true,
                };
            }
            TrailingBody {
                tokens: &tokens[..tokens.len() - 1],
                body: group.stream().into(),
                is_where: false,
            }
        }
        _ => TrailingBody { tokens, body: None, is_where: false },
    }
}

/// Wrapper kind (`WithAttr`/`WithPrefix` half-applied, inner `None`): empty
/// rest keeps the half-applied node, otherwise apply to the parsed remainder.
pub(crate) fn attach_wrapper(
    kind: TyKind, rest: &[TokenTree], trait_name: Option<&Ident>,
) -> Ty {
    let base = Ty { span: proc_macro2::Span::call_site(), kind };
    if rest.is_empty() { base } else { base.apply(parse_primitive(rest, trait_name)) }
}

/// Recursively splits a bracket group (`[A, B]`) into its leaf candidates,
/// distributing nested arrays down to leaves (`[[A, B], C]` → `A`, `B`, `C`).
/// Non-group token sequences are a single candidate.
pub(crate) fn split_arg_candidates(tokens: &[TokenTree]) -> Vec<Vec<TokenTree>> {
    if let [TokenTree::Group(g)] = tokens
        && g.delimiter() == Delimiter::Bracket
    {
        let inner = g.stream().into_iter().collect::<Vec<TokenTree>>();
        split_at_depth0(&inner, ',')
            .iter()
            .flat_map(|e| split_arg_candidates(e))
            .collect()
    } else {
        vec![tokens.to_vec()]
    }
}
