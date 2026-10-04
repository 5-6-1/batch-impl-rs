//! Operator-chain parsing: space-application chains (left-assoc, the
//! successor of the retired `-`) and `.` chains (right-assoc), plus list parsing.
//!
//! Both operators live **between blocks**: the space chain folds blocks
//! (see [`crate::parse::space::parse_block`]) left-associatively, and the
//! `.` chain folds blocks right-associatively with higher precedence:
//!
//! ```text
//! parse_space:  parse_dot (block)*        — left fold
//! parse_dot:    block ('.' parse_dot)*    — right fold
//! ```
//!
//! `Box.u8 u16` = `(Box.u8) u16`; `Box u8 u16` = `(Box<u8>)<u16>`.

use crate::apply::err_ty_at;
use crate::ast::*;
use crate::parse::Ctx;
use crate::parse::parse_primitive;
use crate::parse::space::{cursor_is_dotdot, parse_block, starts_block};
use crate::util::{Cursor, MAX_NEST_DEPTH};
use proc_macro2::TokenTree;

pub(crate) fn parse_item(cursor: &mut Cursor, level: Op, ctx: Ctx<'_>) -> Option<Ty> {
    match level {
        Op::Semi | Op::Comma => loop {
            let before = cursor.pos();
            if let Some(item) = parse_operand(cursor, level, ctx) {
                return item.into();
            }
            if cursor.is_punct(',') {
                cursor.bump();
                // Consecutive commas (`,,`): no operand between the two separators.
                // A trailing single comma is legal (the caller decides that `A,` ends); double comma is a typo.
                if cursor.is_punct(',') {
                    let sp = cursor
                        .peek()
                        .map(|t| t.span())
                        .unwrap_or_else(proc_macro2::Span::call_site);
                    return err_ty_at(
                        "batch-impl: missing operand between consecutive commas `,,` (e.g. `A,,B`)",
                        sp,
                    )
                    .into();
                }
            } else {
                // The general rule, instead of a list of the spellings that are known to
                // disappear: a refusal that *consumed* tokens has dropped a spec, which
                // `docs/reference.md:1060` promises cannot happen silently (`#[batch_impl(^u8)]`,
                // `#[batch_impl(u8, ^u16)]` - and every punctuation nobody has thought of yet,
                // because this arm names no punctuation at all). A refusal that consumed
                // nothing is the ordinary end of the list and stays `None`.
                //
                // `;` is the one documented exception: `#[batch_impl(;)]` is a deliberate
                // empty spelling, so a lone `;` keeps behaving exactly as before.
                let rest = cursor.slice_at(before, usize::MAX);
                if cursor.pos() == before
                    || matches!(rest.first(), Some(TokenTree::Punct(p)) if p.as_char() == ';')
                {
                    return None;
                }
                // Clippy is right that the missing-token case *is* this function's `None`:
                // the cursor only moves forward when something was there.
                let t = rest.first()?;
                return err_ty_at(
                    &format!(
                        "batch-impl: `{t}` cannot start a type — the spec would be dropped \
                         without an impl"
                    ),
                    t.span(),
                )
                .into();
            }
        },
        Op::Space => parse_space_chain(cursor, ctx),
        Op::Dot => parse_dot_chain(cursor, ctx),
        Op::Prim => parse_primitive(cursor.take_rest(), ctx).into(),
    }
}

/// The space-application chain: blocks folded left with `apply` —
/// `Box u8 u16` = `(Box<u8>)<u16>`. A leftover token at a boundary that
/// cannot open a block is an error — a stray `-` gets the retirement message
/// (the exclusion only lives in directive argument lists).
///
/// Empty input returns `None` (legal termination of the enclosing list); a
/// leading `.` is a missing-operand error.
pub(crate) fn parse_space_chain(cursor: &mut Cursor, ctx: Ctx<'_>) -> Option<Ty> {
    if ctx.block_depth == 0 {
        // Reserve transported declarations across the whole spec before
        // either a preceding or following generator can claim their ids.
        crate::parse::reentry::reserve_declarations(cursor.slice_at(cursor.pos(), usize::MAX));
    }
    // One counter for the whole space fold, shared by both calls into the dot chain.
    let mut depth = 0;
    let Some(mut left) = parse_dot_chain_with(cursor, ctx, &mut depth) else {
        if cursor.is_punct('.') {
            return Some(err_ty_at(
                "batch-impl: missing operand before `.` (e.g. `T.U`)",
                cursor.span(),
            ));
        }
        // A token that cannot open a block at the *start* of a type gets a
        // targeted message instead of a silent empty spec (`+A` and `-A` both
        // used to generate 0 impls with no diagnostic at all).
        if let Some(t) = cursor.peek() {
            match t {
                TokenTree::Punct(p) if p.as_char() == '+' => {
                    return Some(err_ty_at(
                        "batch-impl: `+` is not valid at the start of a type (it belongs in a bound, e.g. `T: Clone + Send`)",
                        t.span(),
                    ));
                }
                // The retired infix operator, here in leading position: without
                // this arm the spec parsed as empty and every guarantee about
                // "no silent zero impls" was violated by a single `-`.
                TokenTree::Punct(p) if p.as_char() == '-' => {
                    return Some(err_ty_at(crate::util::RETIRED_DASH, p.span()));
                }
                // Everything else that cannot open a block, in one arm and with no list of
                // spellings: `starts_block` is the parser's own answer to "can this token
                // begin an operand" - the very predicate the loop below uses - so this
                // cannot drift from what parsing actually accepts, and a token nobody has
                // thought of yet is covered the day it can reach here.
                _ if !starts_block(cursor) => {
                    return Some(err_ty_at(
                        &format!(
                            "batch-impl: `{t}` cannot start a type — the spec would be dropped \
                             without an impl"
                        ),
                        t.span(),
                    ));
                }
                _ => {}
            }
        }
        return None;
    };
    let mut count = 1;
    while let Some(t) = cursor.peek() {
        if !starts_block(cursor) {
            return Some(chain_boundary_error(t));
        }
        let Some(right) = parse_dot_chain_with(cursor, ctx, &mut depth) else {
            return Some(err_ty_at(
                "batch-impl: missing operand after the space application",
                t.span(),
            ));
        };
        left = left.apply(right);
        count += 1;
        if count > MAX_NEST_DEPTH {
            return Some(err_ty_at(
                &format!(
                    "batch-impl: space-application chain exceeds {} levels (limit {}); \
                     split the chain into separate impl-specs",
                    count, MAX_NEST_DEPTH,
                ),
                t.span(),
            ));
        }
    }
    Some(left)
}

/// The `.`-chain: blocks folded right with `apply` —
/// `Box.u8 u16` = `(Box<u8>) u16` (`.` binds tighter than the space).
pub(crate) fn parse_dot_chain(cursor: &mut Cursor, ctx: Ctx<'_>) -> Option<Ty> {
    let mut depth = 0;
    parse_dot_chain_with(cursor, ctx, &mut depth)
}

/// The same fold with the caller's operand counter.
///
/// The counter has to be shared rather than created per call: `parse_space_chain` folds
/// with two calls into this function (one at its head, one per iteration), so a local
/// counter walked one long chain as two independent descents from zero, each reaching the
/// limit and each reporting it - two diagnostics for one mistake, one carrying the chain's
/// first dot and one carrying a dot deep inside it.
pub(crate) fn parse_dot_chain_with(
    cursor: &mut Cursor, ctx: Ctx<'_>, depth: &mut usize,
) -> Option<Ty> {
    parse_dot_inner(cursor, ctx, depth)
}

/// The `.`-chain worker: `depth` counts the operands across recursion (the
/// right-assoc fold nests one level per `.`), capped at `MAX_NEST_DEPTH`.
fn parse_dot_inner(cursor: &mut Cursor, ctx: Ctx<'_>, depth: &mut usize) -> Option<Ty> {
    let mut left = parse_block(cursor, ctx)?;
    *depth += 1;
    while cursor.is_punct('.') && !cursor_is_dotdot(cursor) {
        let op_span = cursor.span();
        cursor.bump();
        // The same bound as the check below, applied *before* recursing: ~4800 links used
        // to kill rustc with STATUS_STACK_OVERFLOW and no diagnostic at all. It reports the
        // depth already reached, so the message is identical to the post-recursion one, and
        // it can only fire where the chain still has a link to spend - exactly the case the
        // post-recursion check never reaches, because by then the stack is gone.
        if *depth > MAX_NEST_DEPTH {
            return Some(err_ty_at(
                &format!(
                    "batch-impl: operator chain exceeds {} levels (limit {}); \
                     split the chain into separate impl-specs",
                    *depth, MAX_NEST_DEPTH,
                ),
                op_span,
            ));
        }
        let Some(right) = parse_dot_inner(cursor, ctx, depth) else {
            return Some(err_ty_at("batch-impl: missing operand after `.` (e.g. `T.U`)", op_span));
        };
        left = left.apply(right);
        if *depth > MAX_NEST_DEPTH {
            return Some(err_ty_at(
                &format!(
                    "batch-impl: operator chain exceeds {} levels (limit {}); \
                     split the chain into separate impl-specs",
                    *depth, MAX_NEST_DEPTH,
                ),
                op_span,
            ));
        }
    }
    Some(left)
}

/// Diagnostic for a token that cannot open a block at a chain boundary.
pub(crate) fn chain_boundary_error(t: &TokenTree) -> Ty {
    match t {
        // `-` was retired as the infix apply operator (space took its place);
        // the prefix exclusion lives only in directive argument lists.
        TokenTree::Punct(p) if p.as_char() == '-' => err_ty_at(crate::util::RETIRED_DASH, p.span()),
        // `^` was the power operator before 0.9; the `.N` suffix replaced it.
        // Without this arm the old spelling fell through to the generic
        // "unexpected `^` after the type", which names neither the operator nor
        // the fix (the docs carried `^` examples for several releases).
        TokenTree::Punct(p) if p.as_char() == '^' => {
            err_ty_at(crate::util::RETIRED_CARET, p.span())
        }
        // `where` must be written as a trailing `where{...}` attachment.
        TokenTree::Ident(id) if id == "where" => err_ty_at(
            "batch-impl: `where` is only valid as a trailing `where{...}` attachment",
            id.span(),
        ),
        _ => err_ty_at(&format!("batch-impl: unexpected `{}` after the type", t), t.span()),
    }
}

/// Parse an operand at `level` precedence (up to that level's stop chars, unconsumed).
///
/// Operand bounds come from `scan_stop`; the slice inside the bounds is
/// handed to `parse_item` to recurse at higher precedence.
fn parse_operand(cursor: &mut Cursor, level: Op, ctx: Ctx<'_>) -> Option<Ty> {
    if cursor.at_end() {
        return None;
    }
    let segment = cursor.take_segment(level.stop_chars());
    parse_item(&mut Cursor::new(segment), level.next()?, ctx)
}
