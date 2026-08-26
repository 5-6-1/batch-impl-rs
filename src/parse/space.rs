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

use crate::apply::err_ty_at;
use crate::ast::*;
use crate::parse::blocks::{at_ref_block, literal_block, reference_block, star_block};
use crate::parse::chain::parse_dot_chain;
use crate::parse::generic::empty;
use crate::parse::ident_blocks::ident_block;
use crate::parse::parse_atom::parse_group;
use crate::util::Cursor;
use proc_macro2::{Delimiter, Ident, Spacing, TokenTree};
use quote::quote;

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
    matches!(cursor.peek_op(), Some((crate::util::Op::DotDot, _)))
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
        TokenTree::Punct(p) if p.as_char() == '\'' => {
            // A lifetime reference needs an identifier (`'a`). A lone quote
            // must still be **consumed** — `starts_block` accepts it, and a
            // `None` return here would leave the cursor unmoved, turning
            // every space/bound fold loop that trusts that contract into an
            // infinite append (the second fuzz-OOM root cause).
            if let Some(TokenTree::Ident(id)) = cursor.peek_at(1) {
                let lt = Ident::new(&id.to_string(), id.span());
                cursor.advance(2);
                TyLifetime(crate::parse::blocks::lifetime_tokens(&lt)).to_ty()
            } else {
                cursor.bump();
                crate::apply::err_ty_at(
                    "batch-impl: a lone `'` cannot start a type (a lifetime needs \
                     an identifier, e.g. `'a`)",
                    p.span(),
                )
            }
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
        let pos = cursor.pos();
        let right = parse_dot_chain(cursor, trait_name).unwrap_or_else(empty);
        // Progress invariant: `starts_block` promises a foldable block, but a
        // malformed follower can leave `parse_block` empty-handed and the
        // cursor unmoved — folding again would spin forever appending empties
        // (a fuzz-OOM root cause). Report the stalled token instead.
        if cursor.pos() == pos {
            return err_ty_at(
                &format!("batch-impl: unexpected `{t}` in a type position"),
                t.span(),
            );
        }
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
                let pos = cursor.pos();
                let right = parse_dot_chain(cursor, trait_name).unwrap_or_else(empty);
                // Progress invariant, as in [`parse_return_expr`]: a stalled
                // block-start must end the chain with a diagnostic, never spin.
                if cursor.pos() == pos {
                    return crate::apply::err_ty_at(
                        &format!("batch-impl: unexpected `{t}` in a bound expression"),
                        t.span(),
                    );
                }
                left = left.apply(right);
            }
            _ => break,
        }
    }
    if cursor.is_punct('+') {
        // `+` joins bound elements into a **structured** list — each element
        // stays a `Ty`, so an empty `X<>` inside keeps its identity for the
        // later `X<>` sync pass (a flat token stream would drop the brackets).
        let mut elems = vec![left];
        while cursor.is_punct('+') {
            cursor.bump();
            if let Some(t) = cursor.peek()
                && starts_block(t)
                && !cursor_at_attachment(cursor)
            {
                elems.push(parse_dot_chain(cursor, trait_name).unwrap_or_else(empty));
            }
        }
        return TyBoundList(elems).to_ty();
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
