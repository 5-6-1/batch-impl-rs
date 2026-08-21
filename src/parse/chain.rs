//! Operator-chain parsing: space-application chains (left-assoc, the
//! successor of `-`) and `.` chains (right-assoc), plus list parsing.
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
use crate::parse::parse_primitive;
use crate::parse::space::{cursor_is_dotdot, parse_block, starts_block};
use crate::util::{Cursor, MAX_NEST_DEPTH};
use proc_macro2::{Ident, TokenTree};

pub(crate) fn parse_item(cursor: &mut Cursor, level: Op, trait_name: Option<&Ident>) -> Option<Ty> {
    match level {
        Op::Semi | Op::Comma => loop {
            if let Some(item) = parse_operand(cursor, level, trait_name) {
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
                return None;
            }
        },
        Op::Dash => parse_space_chain(cursor, trait_name),
        Op::Caret => parse_dot_chain(cursor, trait_name),
        Op::Prim => parse_primitive(cursor.take_rest(), trait_name).into(),
    }
}

/// The space-application chain: blocks folded left with `apply` —
/// `Box u8 u16` = `(Box<u8>)<u16>`. A leftover token at a boundary that
/// cannot open a block is an error — a stray `-` gets the retirement message
/// (the exclusion only lives in directive argument lists).
///
/// Empty input returns `None` (legal termination of the enclosing list); a
/// leading `.` is a missing-operand error.
pub(crate) fn parse_space_chain(cursor: &mut Cursor, trait_name: Option<&Ident>) -> Option<Ty> {
    let Some(mut left) = parse_dot_chain(cursor, trait_name) else {
        if cursor.is_punct('.') {
            return Some(err_ty_at(
                "batch-impl: missing operand before `.` (e.g. `T.U`)",
                cursor.span(),
            ));
        }
        // A token that cannot open a block at the *start* of a type gets a
        // targeted message instead of a silent empty spec (`+A` used to
        // generate 0 impls with no diagnostic).
        if let Some(t) = cursor.peek()
            && matches!(t, TokenTree::Punct(p) if p.as_char() == '+')
        {
            return Some(err_ty_at(
                "batch-impl: `+` is not valid at the start of a type (it belongs in a bound, e.g. `T: Clone + Send`)",
                t.span(),
            ));
        }
        return None;
    };
    let mut count = 1;
    while let Some(t) = cursor.peek() {
        if !starts_block(t) {
            return Some(chain_boundary_error(t));
        }
        let Some(right) = parse_dot_chain(cursor, trait_name) else {
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
pub(crate) fn parse_dot_chain(cursor: &mut Cursor, trait_name: Option<&Ident>) -> Option<Ty> {
    let mut depth = 0;
    parse_dot_inner(cursor, trait_name, &mut depth)
}

/// The `.`-chain worker: `depth` counts the operands across recursion (the
/// right-assoc fold nests one level per `.`), capped at `MAX_NEST_DEPTH`.
fn parse_dot_inner(
    cursor: &mut Cursor, trait_name: Option<&Ident>, depth: &mut usize,
) -> Option<Ty> {
    let mut left = parse_block(cursor, trait_name)?;
    *depth += 1;
    while cursor.is_punct('.') && !cursor_is_dotdot(cursor) {
        let op_span = cursor.span();
        cursor.bump();
        let Some(right) = parse_dot_inner(cursor, trait_name, depth) else {
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
fn chain_boundary_error(t: &TokenTree) -> Ty {
    match t {
        // `-` was retired as the infix apply operator (space took its place);
        // the prefix exclusion lives only in directive argument lists.
        TokenTree::Punct(p) if p.as_char() == '-' => err_ty_at(
            "batch-impl: `-` is no longer a type operator (write `A B` or `A.B`; \
             the `-` exclusion only works in directive argument lists like `#fill(@all, -foo)`)",
            p.span(),
        ),
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
fn parse_operand(cursor: &mut Cursor, level: Op, trait_name: Option<&Ident>) -> Option<Ty> {
    if cursor.at_end() {
        return None;
    }
    let segment = cursor.take_segment(level.stop_chars());
    parse_item(&mut Cursor::new(segment), level.next()?, trait_name)
}
