//! Operator-chain parsing: space-application chains (left-assoc, the
//! successor of `-`) and `.` chains (right-assoc), plus operand parsing.

use quote::quote;

use crate::apply::err_ty_at;
use crate::ast::fresh::at_ref_name;
use crate::ast::*;
use crate::parse::{parse_primitive, scan_space_unit, starts_unit, strip_attachments};
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
        Op::Caret => parse_binary_chain(cursor, trait_name),
        Op::Prim => parse_primitive(cursor.take_rest(), trait_name, 0).into(),
    }
}

/// The space-application chain (the `-` operator's successor): units cut at
/// adjacency boundaries by `scan_space_unit` are folded left with `apply` —
/// `Box u8 u16` = `(Box<u8>)<u16>`. Trailing attachments are stripped first
/// (they wrap the whole chain result), the chain runs over the rest, and a
/// leftover token at a boundary that cannot start a unit is an error — a
/// stray `-` gets the retirement message (the exclusion only lives in
/// directive argument lists), a mid-stream `where` gets a targeted one.
///
/// Empty input returns `None` (legal termination of the enclosing list); a
/// bare attachment chain (`{a}{b}`) has the innermost block as its base.
fn parse_space_chain(cursor: &mut Cursor, trait_name: Option<&Ident>) -> Option<Ty> {
    let tokens = cursor.take_rest();
    let (mut attaches, rest) = strip_attachments(tokens);
    // Attachment-chain guard: each attachment nests the type one wrapper
    // level, so a flat chain of bodies overflows the same downstream
    // traversals as a deep operator chain — capped at the same limit.
    if attaches.len() > MAX_NEST_DEPTH {
        return err_ty_at(
            &format!(
                "batch-impl: trailing attachment chain (`{{...}}` / `where{{...}}` / \
                 `impl{{...}}`) exceeds {} levels (limit {}); split into separate impl-specs",
                attaches.len(),
                MAX_NEST_DEPTH,
            ),
            tokens.first().map_or_else(proc_macro2::Span::call_site, |t| t.span()),
        )
        .into();
    }
    let mut ty = if rest.is_empty() {
        // The whole operand is a bare block chain (`{a}{b}`): the innermost
        // block is the "top-level item injection" base (inner `None` mark);
        // empty input = legal termination of the enclosing list.
        attaches.pop()?
    } else {
        space_chain_fold(rest, trait_name)?
    };
    // Apply from inside out (attaches tail = innermost)
    while let Some(block) = attaches.pop() {
        ty = block.apply(ty);
    }
    Some(ty)
}

/// Folds the space units of `rest` left-associatively. Each unit is a full
/// `.`-chain (the scan keeps `.` application inside the unit), parsed at the
/// Caret level; a boundary token that cannot start a unit stops the chain —
/// as an error, since the caller's segment cut already removed the legal
/// terminators (`,` / `;`).
fn space_chain_fold(rest: &[TokenTree], trait_name: Option<&Ident>) -> Option<Ty> {
    let (first_end, first_boundary) = scan_space_unit(rest, 0);
    if first_end == 0 {
        // The first token is not an atom start (a stray `-` / `#` / junk):
        // let the lower levels produce their targeted diagnostics.
        return parse_item(&mut Cursor::new(rest), Op::Caret, trait_name);
    }
    let mut left: Option<Ty> = None;
    let mut pending: Vec<TyTypeParam> = vec![];
    let mut start = 0;
    let mut end = first_end;
    let mut boundary = first_boundary;
    let mut count = 0;
    loop {
        let right = parse_item(&mut Cursor::new(&rest[start..end]), Op::Caret, trait_name)?;
        // A bare angle-group unit between other units is a **generic
        // declaration** (`Trait<A> <T: B> X` / `<'a> <T> X` — the
        // declaration wraps the trait + target), not a trait-arg extend
        // (`Tr <u8>` alone is consumed into the ident's unit and stays
        // trait args). The old skeleton rest-apply produced the same nesting.
        let is_decl = boundary.is_some_and(|b| starts_unit(&rest[b]));
        match right {
            Ty { kind: TyKind::TypeParam(tp), .. } if is_decl => pending.push(tp),
            right => {
                left = Some(match left {
                    Some(l) => l.apply(right),
                    None => right,
                })
            }
        }
        count += 1;
        if count > MAX_NEST_DEPTH {
            return err_ty_at(
                &format!(
                    "batch-impl: space-application chain exceeds {} levels (limit {}); \
                     split the chain into separate impl-specs",
                    count, MAX_NEST_DEPTH,
                ),
                rest[start].span(),
            )
            .into();
        }
        let Some(b) = boundary else { break };
        if !starts_unit(&rest[b]) {
            return chain_boundary_error(&rest[b]).into();
        }
        start = b;
        (end, boundary) = scan_space_unit(rest, start);
    }
    // Declarations wrap outside-in (the earliest one is the outermost), so
    // the innermost wraps first.
    let mut ty = left?;
    for tp in pending.into_iter().rev() {
        ty = TyWithType(tp, ty.into()).to_ty();
    }
    Some(ty)
}

/// Diagnostic for a token that cannot start a space unit at a chain boundary.
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

/// The `.`-chain: left operand → while loop collecting operands → right fold.
/// `.` is right-assoc (`A.B.C = A.(B.C)` — container on the left, nesting inward).
///
/// Chain-length guard: a flat operator chain builds an equally deep `Ty` tree
/// without any group nesting (`.` nests per operand), so the group-level
/// `MAX_NEST_DEPTH` guard would not catch it; the operand count is capped at
/// the same limit here — every downstream recursive traversal
/// (`map_children` / `expand_splat_elems` / rendering) is then depth-bounded.
fn parse_binary_chain(cursor: &mut Cursor, trait_name: Option<&Ident>) -> Option<Ty> {
    // Left operand: `.A` parses to an empty Primitive instead, caught by
    // is_empty_operand. Empty segments error either way.
    let hint = " (e.g. `T.U`)";
    let mut items = match parse_operand(cursor, Op::Caret, trait_name) {
        Some(op) => vec![op],
        None if cursor.at_end() => return None,
        None => {
            return err_ty_at(
                &format!("batch-impl: missing operand before `.`{}", hint),
                cursor_span(cursor),
            )
            .into();
        }
    };
    if is_empty_operand(&items[0]) {
        return err_ty_at(
            &format!("batch-impl: missing operand before `.`{}", hint),
            cursor_span(cursor),
        )
        .into();
    }
    while cursor.is_punct('.') {
        let op_span = cursor_span(cursor);
        cursor.bump();
        let Some(op) = parse_operand(cursor, Op::Caret, trait_name) else {
            return err_ty_at(&format!("batch-impl: missing operand after `.`{}", hint), op_span)
                .into();
        };
        if is_empty_operand(&op) {
            return err_ty_at(&format!("batch-impl: missing operand after `.`{}", hint), op_span)
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
    items.into_iter().rev().reduce(|acc, x| x.apply(acc))
}

/// Span at the cursor's current position (call_site when at end).
fn cursor_span(cursor: &Cursor) -> proc_macro2::Span {
    cursor.peek().map(|t| t.span()).unwrap_or_else(proc_macro2::Span::call_site)
}

/// Whether an operand is empty (when `.` is immediately followed by a depth-0 stop char,
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
