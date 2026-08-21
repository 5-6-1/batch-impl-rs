//! Space-application block parsing: `Box u8` / `HashMap u32 String` —
//! adjacent blocks separated by a space are a left-associative application
//! (the successor of the `-` operator). The space is not a token, so the
//! chain cuts at **block boundaries**: a block is the smallest
//! self-contained type fragment (`&` / `&mut` / `*const` / `fn(...)` /
//! `<...>` / `{...}` / an ident, a group, a number, ...), and the chain
//! folds blocks with `apply`. The `.` operator is a chain-level operator
//! too (right-assoc, higher precedence than the space) — see `chain.rs`.
//!
//! Precedence (low → high): space (left-assoc) < `.` (right-assoc) < block.
//!
//! The block-family implementations live in `blocks.rs`; this file holds the
//! dispatch skeleton and the shared helpers / return-bound expressions.

use crate::ast::*;
use crate::parse::blocks::{at_ref_block, literal_block, reference_block, star_block};
use crate::parse::chain::parse_dot_chain;
use crate::parse::generic::empty;
use crate::parse::ident_blocks::ident_block;
use crate::parse::parse_atom::parse_group;
use crate::util::Cursor;
use proc_macro2::{Delimiter, Ident, Spacing, TokenTree};
use quote::{ToTokens, quote};

/// Whether the token opens a new block: any ident/literal/group or a
/// block-opening punct (`&` `*` `?` `!` `@` `'` `#`). Operators and
/// separators (`.` `,` `;` `-` `:` `+` `>` `=`) do not open blocks.
pub(crate) fn starts_block(t: &TokenTree) -> bool {
    matches!(t, TokenTree::Ident(_) | TokenTree::Literal(_))
        || matches!(t, TokenTree::Group(_))
        || matches!(t, TokenTree::Punct(p)
            if matches!(p.as_char(), '&' | '*' | '?' | '!' | '@' | '\'' | '#'))
}

/// Whether the cursor sits on the first `.` of a `..` range (a Joint `.`
/// whose next token is another `.`).
pub(crate) fn cursor_is_dotdot(cursor: &Cursor) -> bool {
    matches!(cursor.peek(), Some(TokenTree::Punct(p))
        if p.as_char() == '.' && p.spacing() == Spacing::Joint
            && matches!(cursor.peek_at(1), Some(TokenTree::Punct(q)) if q.as_char() == '.'))
}

/// Whether the next tokens open an **attachment** block (`{...}` /
/// `where{...}` / `impl{...}`) — the return-expression chain stops there.
pub(crate) fn cursor_at_attachment(cursor: &Cursor) -> bool {
    match cursor.peek() {
        Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Brace => true,
        Some(TokenTree::Ident(id))
            if (id == "where" || id == "impl")
                && matches!(cursor.peek_at(1), Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Brace) =>
        {
            true
        }
        _ => false,
    }
}

/// Parses one **block** from the cursor: the smallest self-contained type
/// fragment plus its fixed suffixes. Returns `None` when the cursor is not at
/// a block start (an operator / separator / the end).
///
/// A block never swallows the type it would apply to (`&mut u8` is the two
/// blocks `&mut` and `u8`, folded by the chain) — with two exceptions where
/// Rust syntax forces the fragment together: lifetime references
/// (`&'a mut u8`) and the fn family (`fn(u8) -> u8`).
pub(crate) fn parse_block(cursor: &mut Cursor, trait_name: Option<&Ident>) -> Option<Ty> {
    let ty: Ty = match cursor.peek()? {
        // `#[attr]` — attribute block (the chain applies the next block)
        TokenTree::Punct(p)
            if p.as_char() == '#'
                && matches!(cursor.peek_at(1), Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Bracket) =>
        {
            let attr = match cursor.peek_at(1) {
                Some(TokenTree::Group(g)) => g.stream(),
                _ => unreachable!(),
            };
            cursor.advance(2);
            TyWithAttr(TyAttr(attr), None).to_ty()
        }
        // `{body}` code block (incl. `#name{...}` directive products) and
        // the `{! ...}` top-level macro form — chain blocks now.
        TokenTree::Group(g) if g.delimiter() == Delimiter::Brace => {
            let body = g.stream();
            cursor.bump();
            TyWithCode(None, TyCodeBlock(body)).to_ty()
        }
        // `(...)` tuple / `[...]` list-array — the whole group is one block
        TokenTree::Group(g) if g.delimiter() != Delimiter::None => {
            let g = g.clone();
            cursor.bump();
            parse_group(&g, trait_name)
        }
        // `<...>` alone — a generic declaration/args list (TyTypeParam);
        // whether it is a declaration or args is decided by apply.
        TokenTree::Group(g) => {
            let args = g.stream().into_iter().collect::<Vec<_>>();
            cursor.bump();
            crate::parse::generic::parse_angle_bracket_contents(&args, trait_name, true).to_ty()
        }
        // `&` / `&mut` / `&'a` / `&'a mut`
        TokenTree::Punct(p) if p.as_char() == '&' => reference_block(cursor, trait_name),
        // `*const` / `*mut` / `*[...]` / `*(...)`
        TokenTree::Punct(p) if p.as_char() == '*' => star_block(cursor),
        // `@N` position reference
        TokenTree::Punct(p) if p.as_char() == '@' => at_ref_block(cursor),
        // `'a` lifetime
        TokenTree::Punct(p)
            if p.as_char() == '\'' && matches!(cursor.peek_at(1), Some(TokenTree::Ident(_))) =>
        {
            let lt = match cursor.peek_at(1) {
                Some(TokenTree::Ident(id)) => Ident::new(&id.to_string(), id.span()),
                _ => unreachable!(),
            };
            cursor.advance(2);
            TyPrimitive(crate::parse::blocks::lifetime_tokens(&lt)).to_ty()
        }
        // `?` / `!` prefix puncts — swallow the qualified type (passthrough);
        // an attachment block (`{...}` / `where{...}` / `impl{...}`) belongs
        // to the impl, not to the prefixed type (`fn(u8) -> ! { body }`).
        TokenTree::Punct(p) if matches!(p.as_char(), '?' | '!') => {
            let p = p.as_char();
            cursor.bump();
            let inner =
                if cursor_at_attachment(cursor) { None } else { parse_block(cursor, trait_name) }
                    .unwrap_or_else(empty);
            let p_tt = TokenTree::Punct(proc_macro2::Punct::new(p, Spacing::Alone));
            TyPrimitive(quote!(#p_tt #inner)).to_ty()
        }
        // numbers / ranges
        TokenTree::Literal(_) => literal_block(cursor),
        TokenTree::Ident(id) => ident_block(cursor, id.clone(), trait_name),
        _ => return None,
    };
    Some(ty)
}

/// `-> Ret` return expression: blocks folded by the space chain, stopping at
/// an attachment block.
pub(crate) fn parse_return_expr(cursor: &mut Cursor, trait_name: Option<&Ident>) -> Ty {
    let mut left = parse_dot_chain(cursor, trait_name).unwrap_or_else(empty);
    while let Some(t) = cursor.peek() {
        if !starts_block(t) || cursor_at_attachment(cursor) {
            break;
        }
        let right = parse_dot_chain(cursor, trait_name).unwrap_or_else(empty);
        left = left.apply(right);
    }
    left
}

/// A trait bound expression (`Clone + IntoIterator + 'a`): blocks folded by
/// the space chain, then any `+` chain is collected into a passthrough —
/// `+` is a bound operator, not a space application.
pub(crate) fn parse_bound_expr(cursor: &mut Cursor, trait_name: Option<&Ident>) -> Ty {
    let mut left = parse_dot_chain(cursor, trait_name).unwrap_or_else(empty);
    loop {
        match cursor.peek() {
            Some(t) if starts_block(t) && !cursor_at_attachment(cursor) => {
                let right = parse_dot_chain(cursor, trait_name).unwrap_or_else(empty);
                left = left.apply(right);
            }
            _ => break,
        }
    }
    if cursor.is_punct('+') {
        let mut ts = left.to_token_stream();
        while cursor.is_punct('+') {
            ts.extend(cursor.peek().unwrap().to_token_stream());
            cursor.bump();
            if let Some(t) = cursor.peek()
                && starts_block(t)
                && !cursor_at_attachment(cursor)
            {
                let right = parse_dot_chain(cursor, trait_name).unwrap_or_else(empty);
                ts.extend(right.to_token_stream());
            }
        }
        return TyPrimitive(ts).to_ty();
    }
    left
}

/// Consumes the return expression's blocks (used when only the token extent
/// matters — `extern "C" fn` / `Fn(...)` passthrough).
pub(crate) fn parse_return_expr_tokens(cursor: &mut Cursor) {
    if parse_block(cursor, None).is_none() {
        return;
    }
    loop {
        match cursor.peek() {
            Some(t) if starts_block(t) && !cursor_at_attachment(cursor) => {
                let _ = parse_block(cursor, None);
            }
            _ => break,
        }
    }
}
