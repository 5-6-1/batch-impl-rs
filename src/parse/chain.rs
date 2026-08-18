//! Operator-chain parsing: `-`/`^` precedence climbing and operand parsing.

use quote::quote;

use crate::apply::err_ty_at;
use crate::ast::fresh::at_ref_name;
use crate::ast::*;
use crate::parse::parse_primitive;
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
        Op::Dash => parse_binary_chain(cursor, Op::Dash, trait_name, '-', false),
        Op::Caret => parse_binary_chain(cursor, Op::Caret, trait_name, '^', true),
        Op::Prim => parse_primitive(cursor.take_rest(), trait_name, 0).into(),
    }
}

/// Shared skeleton for `-`/`^`: left operand → while loop collecting operands → fold.
/// They differ only in associativity: `-` left-assoc (`A-B-C = (A-B)-C`), `^` right-assoc
/// (`A^B^C = A^(B^C)` — container on the left, nesting inward).
///
/// Chain-length guard: a flat operator chain builds an equally deep `Ty` tree
/// without any group nesting (`^` nests per operand), so the group-level
/// `MAX_NEST_DEPTH` guard would not catch it; the operand count is capped at
/// the same limit here — every downstream recursive traversal
/// (`map_children` / `expand_splat_elems` / rendering) is then depth-bounded.
fn parse_binary_chain(
    cursor: &mut Cursor, level: Op, trait_name: Option<&Ident>, op_punct: char, right_assoc: bool,
) -> Option<Ty> {
    // Left operand: `-A` has an empty left segment — parse_operand returns None only at
    // the end of the cursor (legal termination) or an empty segment (swallowed silently);
    // `^A` parses to an empty Primitive instead, caught by is_empty_operand. Empty
    // segments error either way.
    let hint = if op_punct == '-' { " (e.g. `T-U`)" } else { " (e.g. `T^U`)" };
    let mut items = match parse_operand(cursor, level, trait_name) {
        Some(op) => vec![op],
        None if cursor.at_end() => return None,
        None => {
            return err_ty_at(
                &format!("batch-impl: missing operand before `{}`{}", op_punct, hint),
                cursor_span(cursor),
            )
            .into();
        }
    };
    if is_empty_operand(&items[0]) {
        return err_ty_at(
            &format!("batch-impl: missing operand before `{}`{}", op_punct, hint),
            cursor_span(cursor),
        )
        .into();
    }
    while cursor.is_punct(op_punct) {
        let op_span = cursor_span(cursor);
        cursor.bump();
        let Some(op) = parse_operand(cursor, level, trait_name) else {
            return err_ty_at(
                &format!("batch-impl: missing operand after `{}`{}", op_punct, hint),
                op_span,
            )
            .into();
        };
        if is_empty_operand(&op) {
            return err_ty_at(
                &format!("batch-impl: missing operand after `{}`{}", op_punct, hint),
                op_span,
            )
            .into();
        }
        items.push(op);
        if items.len() > MAX_NEST_DEPTH {
            return err_ty_at(
                &format!(
                    "batch-impl: operator chain exceeds {} levels (limit {}); \
                     split the chain into separate impl-specs",
                    items.len(),
                    MAX_NEST_DEPTH,
                ),
                op_span,
            )
            .into();
        }
    }
    if right_assoc {
        items.into_iter().rev().reduce(|acc, x| x.apply(acc))
    } else {
        items.into_iter().reduce(|acc, x| acc.apply(x))
    }
}

/// Span at the cursor's current position (call_site when at end).
fn cursor_span(cursor: &Cursor) -> proc_macro2::Span {
    cursor.peek().map(|t| t.span()).unwrap_or_else(proc_macro2::Span::call_site)
}

/// Whether an operand is empty (when `^`/`-` is immediately followed by a depth-0 stop char,
/// `take_segment` cuts out an empty slice). An empty operand means a missing operand before or
/// after the operator; `()`/`[]` are real tokens (empty tuple/base), not empty operands.
fn is_empty_operand(ty: &Ty) -> bool {
    matches!(&ty.kind, TyKind::Primitive(p) if p.0.is_empty())
}

/// Parse an operand at `level` precedence (up to that level's stop chars, unconsumed).
///
/// Operand bounds come from `scan_stop` (only `<>` depth, not full Rust type grammar);
/// the slice inside the bounds is handed to `parse_item` to recurse at higher precedence.
fn parse_operand(cursor: &mut Cursor, level: Op, trait_name: Option<&Ident>) -> Option<Ty> {
    if cursor.at_end() {
        return None;
    }
    // `@N` macro-meta position reference: resolved at the type-domain
    // boundary (parse_operand is the type domain's entry) into the fresh
    // name — `@N` → the swept name `_Param_{N}_BatchGen_`, `@g_i` (literal
    // with an underscore, e.g. `0_1`) → the grouped name
    // `_Param_{g}_{i}_BatchGen_` (renumbered by the codegen sweeper along
    // with the generated names). `@trait` never reaches here (expanded at
    // the constant stage / segment level).
    if cursor.is_punct('@') {
        let at_span = cursor.span();
        cursor.bump(); // consume `@`
        return match cursor.peek() {
            Some(TokenTree::Literal(lit)) => {
                match at_ref_name(&lit.to_string()) {
                    Some(name) => {
                        cursor.bump(); // consume the literal
                        // `@0..2` in a type is a range reference — only valid
                        // as a where-predicate subject (`@0..=2: Bound`);
                        // error here instead of a confusing "expected type,
                        // found `..`".
                        if cursor.is_punct('.') {
                            return Some(err_ty_at(
                                "batch-impl: `@N..M` range references are only \
                                 allowed as a where-predicate subject \
                                 (e.g. `where{@0..=2: Clone}`)",
                                at_span,
                            ));
                        }
                        let ident = Ident::new(&name, at_span);
                        Some(TyPrimitive(quote!(#ident)).to_ty().with_span(at_span))
                    }
                    None => Some(err_ty_at(
                        "batch-impl: `@` in a type must be followed by a position \
                         digit (e.g. `@0` or `@0_1`)",
                        at_span,
                    )),
                }
            }
            _ => Some(err_ty_at(
                "batch-impl: `@` in a type must be a position digit (e.g. `@0` or `@0_1`)",
                at_span,
            )),
        };
    }
    let segment = cursor.take_segment(level.stop_chars());
    parse_item(&mut Cursor::new(segment), level.next()?, trait_name)
}
