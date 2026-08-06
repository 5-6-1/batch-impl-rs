//! Parsing layer: DSL precedence-climbing parser and angle-bracket generic parsing.

mod generic;
mod parse_atom;

use proc_macro2::{Ident, TokenStream, TokenTree};

use crate::apply::{err_ty, err_ty_at};
use crate::ast::*;
use crate::parse::generic::{
    is_trait_base, parse_angle_bracket_contents, parse_generic, parse_type_params,
    primitive,
};
use crate::parse::parse_atom::{
    parse_attribute, parse_function, parse_group, parse_prefix, parse_range,
};
use crate::util::Cursor;

// ============================================================
// Operator-level parsing
// ============================================================

/// Parse an expression at `level` precedence, stopping at lower-precedence operators (caller).
/// `Op::Semi` / `Op::Comma` return the first non-empty item; the caller continues after separators.
/// Semi stops before `;` without consuming it, so batch_trait! can detect paragraph boundaries.
pub(crate) fn parse_item(
    cursor: &mut Cursor, level: Op, trait_name: Option<&Ident>,
) -> Option<Ty> {
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
        Op::Prim => parse_primitive(cursor.take_rest(), trait_name).into(),
    }
}

/// Shared skeleton for `-`/`^`: left operand → while loop collecting operands → fold.
/// They differ only in associativity: `-` left-assoc (`A-B-C = (A-B)-C`), `^` right-assoc
/// (`A^B^C = A^(B^C)` — container on the left, nesting inward).
fn parse_binary_chain(
    cursor: &mut Cursor, level: Op, trait_name: Option<&Ident>, op_punct: char,
    right_assoc: bool,
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
fn parse_operand(
    cursor: &mut Cursor, level: Op, trait_name: Option<&Ident>,
) -> Option<Ty> {
    if cursor.at_end() {
        return None;
    }
    let segment = cursor.take_segment(level.stop_chars());
    parse_item(&mut Cursor::new(segment), level.next()?, trait_name)
}

/// DSL parse entry: strips trailing `{...}` code blocks / `where{...}` suffixes,
/// attaching them via apply to the type parsed from the remaining tokens.
///
/// Consecutive attachments (`T{a}{b}` / `T where{...}`) are a **linear chain**; strip by loop
/// removes recursion (deep bodies overflow the stack); iteration removes any depth limit.
pub(crate) fn parse_primitive(
    tokens: &[TokenTree], trait_name: Option<&Ident>,
) -> Ty {
    // Collect attachments outside-in (outer first); `rest` shrinks to the innermost base
    let mut attaches = vec![];
    let mut rest = tokens;
    loop {
        let split = split_trailing_body(rest);
        match (split.body, split.is_where) {
            (Some(body), false) => {
                attaches.push(TyWithCode(None, TyCodeBlock(body)).into());
                rest = split.tokens;
            }
            (Some(w), true) => {
                attaches.push(TyWithWhere(None, TyWhere(w)).into());
                rest = split.tokens;
            }
            _ => break,
        }
    }
    let mut ty = if rest.is_empty() {
        // The whole operand is a bare block chain (`{a}{b}`): the innermost block is the "top-level item
        // injection" base (inner `None` mark); empty attaches = empty input, so parse atomically
        match attaches.pop() {
            Some(inner) => inner,
            None => parse_primary(rest, trait_name),
        }
    } else {
        parse_primary(rest, trait_name)
    };
    // Apply from inside out (attaches tail = innermost)
    while let Some(block) = attaches.pop() {
        ty = block.apply(ty);
    }
    ty
}

// ============================================================
// Atom-level parsing
// ============================================================

/// Result of stripping the trailing `{...}`
struct TrailingBody<'a> {
    /// Remaining tokens after stripping the trailing code block
    tokens: &'a [TokenTree],
    /// The stripped body; `None` means there is no trailing code block
    body: Option<TokenStream>,
    /// `true` when the body is a `where{...}` predicate suffix
    is_where: bool,
}

/// Split off a trailing `{...}` code block (`macro!{...}` excluded; `where{...}` is a predicate)
fn split_trailing_body(tokens: &[TokenTree]) -> TrailingBody<'_> {
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
fn attach_wrapper(
    kind: TyKind, rest: &[TokenTree], trait_name: Option<&Ident>,
) -> Ty {
    let base = Ty::new(proc_macro2::Span::call_site(), kind);
    if rest.is_empty() { base } else { base.apply(parse_primitive(rest, trait_name)) }
}

/// Parse one "atom" expression: attribute → function → prefix → range → number → group →
/// generic → type params → primitive fallback
fn parse_primary(tokens: &[TokenTree], trait_name: Option<&Ident>) -> Ty {
    if let Some((attr, rest)) = parse_attribute(tokens) {
        return attach_wrapper(
            TyWithAttr(TyAttr(attr), None).into(),
            rest,
            trait_name,
        );
    }

    if let Some(function) = parse_function(tokens, trait_name) {
        return function;
    }

    // Bare `fn` (no params): `fn^(A,B)` gets its args filled in later by the `^` operator
    if let [TokenTree::Ident(name)] = tokens
        && name == "fn"
    {
        return TyFn(None, None, false).into();
    }

    if let Some((prefix, rest)) = parse_prefix(tokens) {
        // `unsafe` prefix disambiguation:
        // - bare `unsafe` (rest empty) → unsafe impl marker (unsafe^T / unsafe-T), passthrough verbatim
        // - `unsafe fn...` → unsafe fn type (TyFn.is_unsafe set)
        // - `unsafe X` (X not fn) → error: in Rust, unsafe only qualifies fn types; writing it next to
        //   any other type is almost certainly a forgotten `^` (unsafe^Vec<T>)
        if matches!(prefix, TyPrefix::Unsafe) && !rest.is_empty() {
            if matches!(rest.first(), Some(TokenTree::Ident(f)) if f == "fn") {
                let inner = parse_primitive(rest, trait_name);
                return match inner.kind {
                    TyKind::Fn(mut f) => {
                        f.2 = true;
                        Ty::new(inner.span, TyKind::Fn(f))
                    }
                    // rest starts with `fn`, so parse_primitive must return TyFn; defensive fallback
                    other => Ty::new(inner.span, other),
                };
            }
            return err_ty(
                "batch-impl: `unsafe` can only qualify a fn type (e.g. `unsafe fn(u32) -> u32`) \
or act as a bare impl marker (e.g. `unsafe^T`)",
            );
        }
        let inner =
            attach_wrapper(TyWithPrefix(prefix, None).into(), rest, trait_name);
        return inner;
    }

    if let Some(range) = parse_range(tokens) {
        return range;
    }

    if let [TokenTree::Literal(literal)] = tokens
        && let Ok(number) = literal.to_string().parse()
    {
        return TyNum(number).into();
    }

    // An angle-bracket group (`delimiter![<>]`) is a generic list; must go through
    // parse_type_params (else `HashMap^<A,B>`'s right operand is swallowed as empty by parse_group)
    if let [TokenTree::Group(group)] = tokens
        && group.delimiter() != delimiter![<>]
    {
        return parse_group(group, trait_name);
    }

    if let Some((base, args, rest)) = parse_generic(tokens) {
        let args_vec: Vec<_> = args.into_iter().collect();
        let params = parse_angle_bracket_contents(&args_vec, trait_name);
        let generic = if is_trait_base(&base, trait_name) {
            TyTrait(base.iter().cloned().collect(), params).into()
        } else {
            // rest non-empty and not an angle-bracket group (`Vec<T><U>` = chained generics, via apply):
            // anything else (e.g. `Vec<T>U`) is treated as a passthrough
            if !rest.is_empty()
                && !matches!(rest.first(), Some(TokenTree::Group(g)) if g.delimiter() == delimiter![<>])
            {
                return primitive(tokens);
            }
            TyGeneric(primitive(&base).into(), params).into()
        };
        return if rest.is_empty() {
            generic
        } else {
            generic.apply(parse_primitive(&rest, trait_name))
        };
    }

    if let Some((args, rest)) = parse_type_params(tokens) {
        let args_vec: Vec<_> = args.into_iter().collect();
        let params = parse_angle_bracket_contents(&args_vec, trait_name);
        let params = params.into();
        return if rest.is_empty() {
            params
        } else {
            params.apply(parse_primitive(&rest, trait_name))
        };
    }

    primitive(tokens)
}
