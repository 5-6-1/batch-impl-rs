use proc_macro2::{Ident, Spacing, TokenTree};

use crate::util::is_punct;

/// Detect a `# Path :` path prefix at the start of attr.
///
/// Rule: when it starts with `#` + `Ident` + (`::` `Ident`)+ + `:`, return
/// `Some((path_tokens, last_ident, remaining tokens))`, where `path_tokens` excludes the
/// leading `#` and trailing `:`; otherwise return `None` and the caller treats the whole
/// attr as plain DSL.
///
/// **Requires at least one `::`**: `#Display: ...` does not match (a single-segment trait
/// name is given implicitly by the dummy trait ident, so no path prefix is needed). This
/// avoids the ambiguity of `#Display: ...` being mistaken for a path prefix that swallows
/// `Display`.
pub(crate) fn try_parse_path_prefix(
    tokens: &[TokenTree],
) -> Option<(Vec<TokenTree>, Option<Ident>, Vec<TokenTree>)> {
    // Shape: # Ident (:: Ident)+ :   minimum 5 tokens: # Ident :: Ident :
    if tokens.len() < 5 {
        return None;
    }
    if !is_punct(&tokens[0], '#') {
        return None;
    }
    if !matches!(&tokens[1], TokenTree::Ident(_)) {
        return None;
    }
    // State machine:
    //   expect_sep=true  → expect `::` (two `:` Puncts, the second `:` may be Alone)
    //   expect_sep=false → expect Ident (right after `::`)
    // Start: just read the first Ident, expect `::`.
    let mut i = 2usize;
    let mut expect_sep = true;
    let mut saw_double_colon = false;
    let mut last_ident = None;
    loop {
        match tokens.get(i) {
            // `::`: must be two consecutive `:`, with the first `:` Spacing::Joint
            Some(TokenTree::Punct(p)) if p.as_char() == ':' && expect_sep => {
                match tokens.get(i + 1) {
                    Some(TokenTree::Punct(p2))
                        if p2.as_char() == ':' && p.spacing() == Spacing::Joint =>
                    {
                        i += 2;
                        expect_sep = false;
                        saw_double_colon = true;
                    }
                    // Single `:` terminator — accepted only after at least one `::`
                    _ if saw_double_colon => {
                        let path = tokens[1..i].to_vec();
                        let rest = tokens[i + 1..].to_vec();
                        return (path, last_ident, rest).into();
                    }
                    // Otherwise falls through to the `_ => return None` below
                    _ => return None,
                }
            }
            // Expected Ident (right after `::`)
            Some(TokenTree::Ident(id)) if !expect_sep => {
                i += 1;
                last_ident = id.clone().into();
                expect_sep = true;
            }
            _ => return None,
        }
    }
}
