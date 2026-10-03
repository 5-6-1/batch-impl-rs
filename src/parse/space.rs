//! Space-application block parsing: `Box u8` / `HashMap u32 String` —
//! adjacent blocks separated by a space are a left-associative application
//! (the successor of the `-` operator). The space is not a token, so the
//! chain cuts at **block boundaries**: a block is the smallest
//! self-contained type fragment (`&` / `&mut` / `*const` / `fn(...)` /
//! `<...>` / `{...}` / a `::path`, an ident, a group, a number, ...), and the chain
//! folds blocks with `apply`. The `.` operator is a chain-level operator
//! too (right-assoc, higher precedence than the space) — see `chain.rs`.
//!
//! Precedence (low → high): space (left-assoc) < `.` (right-assoc) < block.
//!
//! The block-family implementations live in `blocks.rs`; this file holds the
//! dispatch skeleton and the shared helpers / return-bound expressions.

use crate::apply::err_ty_at;
use crate::ast::*;
use crate::parse::Ctx;
use crate::parse::blocks::{at_ref_block, literal_block, reference_block, star_block};
use crate::parse::chain::parse_dot_chain;
use crate::parse::generic::empty;
use crate::parse::ident_blocks::ident_block;
use crate::parse::parse_atom::parse_group;
use crate::util::Cursor;
use proc_macro2::{Ident, Spacing, TokenTree};
use quote::quote;

/// Whether the cursor opens a new block: any ident/literal/group or a
/// block-opening punct (`&` `*` `?` `!` `@` `'` `#` `::`). Operators and
/// separators (`.` `,` `;` `-` `:` `+` `>` `=`) do not open blocks — and a
/// **lone** `:` does not either (only the `::` of a global path, read off the
/// compound-operator dictionary, starts a block).
pub(crate) fn starts_block(cursor: &Cursor) -> bool {
    match cursor.peek() {
        Some(TokenTree::Ident(_) | TokenTree::Literal(_) | TokenTree::Group(_)) => true,
        Some(TokenTree::Punct(p)) => match p.as_char() {
            '&' | '*' | '?' | '!' | '@' | '\'' | '#' => true,
            ':' => matches!(cursor.peek_op(), Some((crate::util::Op::ColonColon, _))),
            _ => false,
        },
        None => false,
    }
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
        Some(TokenTree::Group(g)) if g.delimiter() == delimiter![{}] => true,
        Some(TokenTree::Ident(id))
            if (id == "where" || id == "impl")
                && matches!(cursor.peek_at(1), Some(TokenTree::Group(g)) if g.delimiter() == delimiter![{}]) =>
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
/// blocks `&mut` and `u8`, folded by the chain), except for lifetime references
/// (`&'a mut u8`), the fn family (`fn(u8) -> u8`) and the Pack prefix `*T`,
/// which consumes exactly one following block.
pub(crate) fn parse_block(cursor: &mut Cursor, ctx: Ctx<'_>) -> Option<Ty> {
    cursor.peek()?;
    // Flat prefix chains (`***...T`, `?*?*...T`) recurse without adding a
    // token group, so the preprocessor's delimiter-depth guard cannot see them.
    if ctx.block_depth >= crate::util::MAX_NEST_DEPTH {
        return Some(
            TyError(crate::util::depth_err(cursor.take_rest(), " in type block parsing")).to_ty(),
        );
    }
    let ctx = Ctx { block_depth: ctx.block_depth + 1, ..ctx };
    let ty = match cursor.peek()? {
        // `#[attr]` — attribute block (the chain applies the next block)
        TokenTree::Punct(p)
            if p.as_char() == '#' && matches!(cursor.peek_at(1), Some(TokenTree::Group(_))) =>
        {
            let Some(attr_group) = cursor.peek_group_at(1, delimiter![[]]) else {
                // `#` followed by a non-bracket group is a stray: a directive needs
                // its name (`#name{…}`), and falling through here used to drop the
                // whole spec silently — the trait was emitted, zero impls, and no
                // diagnostic pointed at the attribute.
                let span = p.span();
                cursor.advance(2);
                return Some(err_ty_at(
                    "batch-impl: `#` needs a directive name (`#name{…}`); to attach an \
                     attribute write `#[…]`",
                    span,
                ));
            };
            let attr = attr_group.stream();
            cursor.advance(2);
            TyWithAttr(TyAttr(attr), None).to_ty()
        }
        // `{body}` code block (incl. `#name{...}` directive products) and
        // the `{! ...}` top-level macro form — chain blocks now.
        TokenTree::Group(g) if g.delimiter() == delimiter![{}] => {
            let body = g.stream();
            cursor.bump();
            TyWithCode(None, TyCodeBlock(body)).to_ty()
        }
        // `(...)` tuple / `[...]` list-array — the whole group is one block
        TokenTree::Group(g) if g.delimiter() != delimiter![<>] => {
            let g = g.clone();
            cursor.bump();
            parse_group(&g, ctx)
        }
        // `<...>` alone — a generic declaration/args list (TyTypeParam);
        // whether it is a declaration or args is decided by apply. A `<...>`
        // **immediately followed by `::`** whose content has a depth-0 `as` is a
        // qualified-self head instead (`<T as Tr>::Assoc`): the pairing pass made
        // the group, so the `::`-tail has nowhere else to attach.
        TokenTree::Group(g) => {
            let args = g.stream().into_iter().collect::<Vec<_>>();
            cursor.bump();
            if matches!(cursor.peek_op(), Some((crate::util::Op::ColonColon, _)))
                && let Some((ty_tokens, trait_)) = crate::parse::split_projection(&args)
            {
                let head_ty =
                    crate::parse::parse_item(&mut Cursor::new(&ty_tokens), Op::Comma, ctx)
                        .unwrap_or_else(crate::parse::generic::empty);
                return Some(crate::parse::ident_blocks::qualified_tail(
                    cursor,
                    QualifiedHead::Projection(head_ty.into(), trait_),
                ));
            }
            crate::parse::generic::parse_angle_bracket_contents(
                &args,
                ctx,
                crate::parse::generic::ArgsPosition::Declaration,
            )
            .to_ty()
        }
        // `&` / `&mut` / `&'a` / `&'a mut`
        TokenTree::Punct(p) if p.as_char() == '&' => reference_block(cursor, ctx),
        // `*const` / `*mut` take precedence over the `*T` pack prefix.
        TokenTree::Punct(p) if p.as_char() == '*' => star_block(cursor, ctx),
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
                err_ty_at(
                    "batch-impl: a lone `'` cannot start a type (a lifetime needs \
                     an identifier, e.g. `'a`)",
                    p.span(),
                )
            }
        }
        // `?` / `!` prefix puncts — retain the qualified type structurally;
        // an attachment block (`{...}` / `where{...}` / `impl{...}`) belongs
        // to the impl, not to the prefixed type (`fn(u8) -> ! { body }`). The
        // swallowed type is a **sub-type position**, so the bound flag stops
        // here (`T: ?Vec<Item = u8>` stays a plain arg list).
        TokenTree::Punct(p) if matches!(p.as_char(), '?' | '!') => {
            let p = p.as_char();
            cursor.bump();
            let inner =
                if cursor_at_attachment(cursor) { None } else { parse_block(cursor, ctx.plain()) }
                    .unwrap_or_else(empty);
            let p_tt = TokenTree::Punct(proc_macro2::Punct::new(p, Spacing::Alone));
            TyPrefixed(quote!(#p_tt), inner.into()).to_ty()
        }
        // numbers / ranges
        TokenTree::Literal(_) => literal_block(cursor),
        TokenTree::Ident(id) => ident_block(cursor, id.clone(), ctx),
        // `::path` — a leading `::` makes the path global (`::std::vec::Vec<u8>`):
        // the `::` is part of the ident block, so the ident parser keeps its
        // segments / turbofish / args handling. A **lone** `:` is a separator,
        // not a block start.
        TokenTree::Punct(p)
            if p.as_char() == ':'
                && matches!(cursor.peek_op(), Some((crate::util::Op::ColonColon, _))) =>
        {
            return crate::parse::ident_blocks::global_path_block(cursor, ctx);
        }
        // A stray `#` with no group after it (the preprocessor already consumed
        // every real `#name{…}`): silently returning `None` here dropped the spec.
        TokenTree::Punct(p)
            if p.as_char() == '#' && !matches!(cursor.peek_at(1), Some(TokenTree::Ident(_))) =>
        {
            cursor.bump();
            err_ty_at(
                "batch-impl: `#` must start a directive with a name (`#name{…}`) or an \
                 attribute (`#[…]`)",
                p.span(),
            )
        }
        _ => return None,
    };
    Some(ty)
}

/// `-> Ret` return expression: blocks folded by the space chain, stopping at
/// an attachment block.
pub(crate) fn parse_return_expr(cursor: &mut Cursor, ctx: Ctx<'_>) -> Ty {
    let mut left = parse_dot_chain(cursor, ctx).unwrap_or_else(empty);
    while let Some(t) = cursor.peek() {
        if !starts_block(cursor) || cursor_at_attachment(cursor) {
            break;
        }
        let pos = cursor.pos();
        let right = parse_dot_chain(cursor, ctx).unwrap_or_else(empty);
        // Progress invariant: `starts_block` promises a foldable block, but a
        // malformed follower can leave `parse_block` empty-handed and the
        // cursor unmoved — folding again would spin forever appending empties
        // (a fuzz-OOM root cause). Report the stalled token instead.
        if cursor.pos() == pos {
            let (text, span) = (t.to_string(), t.span());
            cursor.bump();
            return err_ty_at(&format!("batch-impl: unexpected `{text}` in a type position"), span);
        }
        left = left.apply(right);
    }
    left
}

/// A trait bound expression (`Clone + IntoIterator + 'a`): blocks folded by
/// the space chain, then any `+` chain is collected into a structured list —
/// `+` is a bound operator, not a space application.
///
/// The bound flag covers the **head** of the first element only. A follower of
/// the space fold is an apply **operand** (`T: Conv Item` folds to `Conv<Item>`),
/// i.e. a sub-type position, so the flag stops there; the `+` elements are bound
/// elements again and keep it. (`T: Clone Vec<Item = u8>` is not valid Rust
/// anyway — rustc reports a parse error at `Vec` — so nothing legal is affected.)
pub(crate) fn parse_bound_expr(cursor: &mut Cursor, ctx: Ctx<'_>) -> Ty {
    // A bound is the position where associated-type bindings are legal on
    // **any** trait path (`T: Iterator<Item = u8>`) — the flag rides the
    // context down to the ident parser.
    let ctx = ctx.in_bound();
    let mut left = parse_dot_chain(cursor, ctx).unwrap_or_else(empty);
    loop {
        match cursor.peek() {
            Some(t) if starts_block(cursor) && !cursor_at_attachment(cursor) => {
                let pos = cursor.pos();
                let right = parse_dot_chain(cursor, ctx.plain()).unwrap_or_else(empty);
                // Progress invariant, as in [`parse_return_expr`]: a stalled
                // block-start must end the chain with a diagnostic, never spin.
                if cursor.pos() == pos {
                    return err_ty_at(
                        &format!("batch-impl: unexpected `{t}` in a bound expression"),
                        t.span(),
                    );
                }
                left = left.apply(right);
            }
            _ => {
                // A retired operator must never be dropped silently: the bound
                // fold used to break here, so `T: Tr^u8` rendered `T: Tr` and
                // the `^u8` vanished (measured while adding the `^` retirement
                // message). Only the retired puncts are checked — every other
                // leftover is a legitimate boundary (`,`, `=`, the end of the
                // chunk).
                if let Some(t) = cursor.peek()
                    && matches!(t, TokenTree::Punct(p) if p.as_char() == '^' || p.as_char() == '-')
                {
                    return crate::parse::chain::chain_boundary_error(t);
                }
                break;
            }
        }
    }
    if cursor.is_punct('+') {
        // `+` joins bound elements into a **structured** list — each element
        // stays a `Ty`, so an empty `X<>` inside keeps its identity for the
        // later `X<>` sync pass (a flat token stream would drop the brackets).
        let mut elems = vec![left];
        while cursor.is_punct('+') {
            cursor.bump();
            if starts_block(cursor) && !cursor_at_attachment(cursor) {
                elems.push(parse_dot_chain(cursor, ctx).unwrap_or_else(empty));
            }
        }
        return TyBoundList(elems).to_ty();
    }
    left
}
