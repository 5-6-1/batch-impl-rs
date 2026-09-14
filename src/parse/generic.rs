//! Generic and angle-bracket parsing module.
//!
//! Provides matching and parsing of `<...>` generic parameters plus related helpers.

use proc_macro2::{Spacing, TokenTree};

use quote::quote;

use crate::apply::err_ty_at;
use crate::ast::*;
use crate::parse::resolve_at_refs;
use crate::parse::{Ctx, parse_item};
use crate::util::{
    Cursor, compile_error_ty, is_single_colon, scan_stop, slice_from, slice_upto, span_at,
};

// ============================================================
// Angle brackets and generic parameters
// ============================================================

/// Split by separator (angle brackets are already paired into opaque groups, so flat split)
///
/// **Note**: a flat `<A, B>` would be wrongly cut by a depth-0 comma split (`T: Two<A, B>` →
/// `T: Two<A` / `B>`); if the macro ever generates generic-group contents containing angle
/// brackets, they must be paired (`Group::new(delimiter![<>], ...)`) before insertion,
/// never scattered as flat `<...>`.
pub(crate) fn split_at_depth0(tokens: &[TokenTree], separator: char) -> Vec<&[TokenTree]> {
    let mut chunks = vec![];
    let mut rest = tokens;
    while let Some(index) = scan_stop(rest, &[separator]) {
        chunks.push(slice_upto(rest, index));
        rest = slice_from(rest, index + 1);
    }
    chunks.push(rest);
    chunks
}

/// Find the first `:` that is not part of `::` (used to split `T: Bound`)
fn find_colon_at_depth0(tokens: &[TokenTree]) -> Option<usize> {
    scan_stop(tokens, &[':']).filter(|&index| is_single_colon(tokens, index))
}

/// Where an angle-bracket list sits — the **single authority** for what its
/// chunks may contain. Bounds and associated-type bindings are *not* valid in the
/// same positions, and the answer is the position, not the shape of the list:
///
/// * [`ArgsPosition::TraitPath`] — a trait application (`Conv<Item = u32> X`) or a
///   **bound** (`T: Iterator<Item = u8>`, `dyn …`, `for<'a> …`): bounds and
///   bindings both belong;
/// * [`ArgsPosition::Declaration`] — a generic-declaration block
///   (`<T: Clone, Item = u8> Foo`): bounds are the declaration's own, a binding
///   declares nothing — it is reported and pointed at the trait-application
///   spelling (`Trait<Item = u8> Foo`);
/// * [`ArgsPosition::PlainType`] — a plain type's argument list (`Vec<u8>`):
///   neither is valid.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArgsPosition {
    TraitPath,
    Declaration,
    PlainType,
}

/// Parse `<T: Clone, U, Item=V>` contents: parameter list + associated-type bindings
pub(crate) fn parse_angle_bracket_contents(
    tokens: &[TokenTree], ctx: Ctx<'_>, position: ArgsPosition,
) -> TyTypeParam {
    // `allow_special`: bindings (`Item = u32`) and bounds (`T: Clone`) are
    // valid on a trait path (`Conv<Item = u32> X`) and in a **bound** position
    // (`T: Iterator<Item = u8>` / `dyn Iterator<Item = u8>`) — a concrete
    // type's args are a plain type list, so `=`/`:` there is a usage error
    // (previously the bound was silently dropped and a struct binding rendered
    // invalid code). A generic **declaration** allows bounds but not bindings
    // (see [`ArgsPosition`]).
    let allow_special = position != ArgsPosition::PlainType;
    //
    // The recursion into a chunk is a **plain** one: the chunks are types
    // (binding values / args), so the bound flag must not leak into them.
    let ctx = ctx.plain();
    let mut params = vec![];
    let mut bindings = vec![];
    for chunk in split_at_depth0(tokens, ',') {
        if chunk.is_empty() {
            continue;
        }
        // Splat args need no special case: `Foo<*(a,b)>` falls through to
        // the default path below, which keeps the `*(a,b)` token as one
        // generic arg — the codegen postprocess flattens it into `Foo<a,b>`
        // at render. A generator splat there (`Foo<*(().N)>`) hoists its
        // fresh declaration out of the args (flat_splat_params) — same rule
        // as the trait-arg position (0.7.2).
        // `@N` position refs inside angle args (`Box<@0>`) are not parsed as
        // types (flat token splitting) — resolve them to fresh names here.
        // A resolution error yields a `compile_error!` token stream that
        // surfaces when the impl header is rendered.
        if let Some(eq) = scan_stop(chunk, &['=']) {
            if position == ArgsPosition::Declaration {
                // A declaration block declares **parameters**; an associated-type
                // binding there declares nothing (it used to be honoured as the
                // impl's associated type, which is a spelling the maintainer never
                // modelled — the binding belongs on the trait application). Report
                // with the spelling that works instead of inventing one.
                params.push((
                    Box::new(err_ty_at(
                        "batch-impl: an associated-type binding belongs on the trait \
                         application — write `Trait<Item = u8> Target`, not \
                         `<Item = u8> Target` (a `<>` block declares parameters)",
                        span_at(chunk, eq),
                    )),
                    None,
                ));
            } else if allow_special {
                let name = slice_upto(chunk, eq);
                let name_ty = TyPrimitive(name.iter().cloned().collect()).to_ty();
                let value = match resolve_at_refs(slice_from(chunk, eq + 1)) {
                    Ok(v) if v.is_empty() => TyPrimitive(compile_error_ty(
                        "batch-impl: binding `Item =` missing a value (write `Item = u32`)",
                        span_at(chunk, eq),
                    ))
                    .to_ty(),
                    Ok(v) => {
                        let parsed =
                            parse_item(&mut Cursor::new(&v), Op::Space, ctx).unwrap_or_else(empty);
                        // A binding takes exactly **one** type — a splat is a
                        // parameter-position list with no flattening target in
                        // a binding (same ruling as a bare splat as a
                        // where-predicate subject: constraints/values are not
                        // lists). Distribute via a spec list instead.
                        match parsed.kind {
                            TyKind::Splat(_) => err_ty_at(
                                "batch-impl: a splat cannot be an associated-type binding \
                                 value (`Item = *(A,B)` — bindings take exactly one type; \
                                 distribute via a spec list like `[Tr<Item=A>, Tr<Item=B>]`)",
                                parsed.span,
                            ),
                            _ => parsed,
                        }
                    }
                    // A malformed `@` reference is an **error node**, not a
                    // primitive carrying an item-form `compile_error!(…);`: in a
                    // type position the `;` is a syntax error, so rustc reported
                    // `expected one of ',' or '>'` and the macro's own message
                    // never appeared (second review round, F2).
                    Err(e) => e.into_ty(),
                };
                bindings.push((Box::new(name_ty), Box::new(value)));
            } else {
                params.push((
                    Box::new(
                        TyPrimitive(compile_error_ty(
                            "batch-impl: binding args (`Item = u32`) are only valid on a trait path (`Conv<Item = u32> X`) or in a bound (`T: Iterator<Item = u8>`) — a concrete type's args are a plain type list",
                            span_at(chunk, eq),
                        ))
                        .to_ty(),
                    ),
                    None,
                ));
            }
        } else if let Some(colon) = find_colon_at_depth0(chunk) {
            if allow_special {
                params.push((
                    Box::new(
                        TyPrimitive(slice_upto(chunk, colon).iter().cloned().collect()).to_ty(),
                    ),
                    Some(Box::new(if slice_from(chunk, colon + 1).is_empty() {
                        TyPrimitive(compile_error_ty(
                            "batch-impl: bound `T:` missing a bound (write `T: Clone`)",
                            span_at(chunk, colon),
                        ))
                        .to_ty()
                    } else {
                        // A bound is a `+`-chain (`Clone + Send + 'a`) — the
                        // bound operator is not a space application.
                        crate::parse::space::parse_bound_expr(
                            &mut Cursor::new(slice_from(chunk, colon + 1)),
                            ctx,
                        )
                    })),
                ));
            } else {
                params.push((
                    Box::new(
                        TyPrimitive(compile_error_ty(
                            "batch-impl: bound args (`T: Clone`) are only valid on a trait path, in a generic declaration (`<T: Clone> Foo`) or in a bound — a concrete type's args are a plain type list",
                            span_at(chunk, colon),
                        ))
                        .to_ty(),
                    ),
                    None,
                ));
            }
        } else {
            let name = match resolve_at_refs(chunk) {
                Ok(v) => parse_item(&mut Cursor::new(&v), Op::Space, ctx).unwrap_or_else(empty),
                // Same channel rule as the binding case above (`F2`).
                Err(e) => e.into_ty(),
            };
            params.push((Box::new(name), None));
        }
    }
    TyTypeParam { params, bindings }
}

// ============================================================
// Fallbacks
// ============================================================

/// Wrap a token sequence as a Primitive passthrough node (any unrecognized
/// type lands here). Four guards run first — each returns a targeted
/// diagnostic for a token with no legal role in a type position at depth 0 —
/// so the passthrough never renders invalid Rust without guidance.
pub(crate) fn primitive(tokens: &[TokenTree]) -> Ty {
    let span = tokens.first().map(|t| t.span()).unwrap_or_else(proc_macro2::Span::call_site);
    if let Some(e) = validate_stray_punct(tokens) {
        return e;
    }
    if let Some(e) = validate_range(tokens) {
        return e;
    }
    if let Some(e) = validate_start_punct(tokens) {
        return e;
    }
    TyPrimitive(tokens.iter().cloned().collect()).to_ty().with_span(span)
}

/// `;`/`=`/`@`/`#`/`-` at depth 0 in a type position are always invalid — `;` is
/// the `batch_trait!` segment boundary; `=`/`@`/`#` have no legal role in a
/// type (they belong inside `<...>` or before parsing); `-` was retired as
/// the infix apply operator (space took its place; the exclusion survives
/// only in directive argument lists). The `=` of `..=` is part of the range
/// operator, not a binding, so a leftover after an earlier error must not
/// cascade a second diagnostic; the `-` of `->` is part of the fn arrow.
fn validate_stray_punct(tokens: &[TokenTree]) -> Option<Ty> {
    for (i, tt) in tokens.iter().enumerate() {
        // The `=` of `..=` is part of the range operator, not a binding, so a
        // leftover after an earlier error must not cascade a second
        // diagnostic; the `-` of `->` is part of the fn arrow. Both read off
        // the shared operator dictionary (a compound operator's members are
        // one unit).
        let is_range_inclusive = matches!(
            i.checked_sub(2).and_then(|j| crate::util::read_op(tokens, j)),
            Some((crate::util::Op::DotDotEq, _))
        );
        let msg = if is_range_inclusive {
            None
        } else if let TokenTree::Punct(p) = tt {
            match crate::util::read_op(tokens, i) {
                Some((crate::util::Op::Semicolon, _)) => Some(
                    "batch-impl: `;` is not valid in a type (it is the `batch_trait!` \
                     segment boundary; in `#[batch_impl]` specs are separated by `,`)",
                ),
                Some((crate::util::Op::Eq, _)) => Some(
                    "batch-impl: `=` is not valid in a type position (associated-type \
                     bindings like `Item = u32` belong inside a trait path's `<...>`)",
                ),
                Some((crate::util::Op::At, _)) => Some(
                    "batch-impl: `@` inside a type (position references like `@0` must \
                     start an operand, e.g. `T.@0`)",
                ),
                // a lone `-` is the retired operator; `->` (the fn arrow) is
                // parsed by parse_function
                Some((crate::util::Op::Minus, _)) => Some(crate::util::RETIRED_DASH),
                // `#` is outside the operator alphabet but still stray
                _ if p.as_char() == '#' => Some(
                    "batch-impl: `#` inside a type (attributes belong at the spec start \
                     as `#[...].T`; directives are expanded before parsing)",
                ),
                _ => None,
            }
        } else {
            None
        };
        if let Some(msg) = msg {
            return Some(err_ty_at(msg, tt.span()));
        }
    }
    None
}

/// A `.` in a type position that `parse_range` did not consume means the
/// range endpoints were not integer literals (`1..x`, `A..B`) — report
/// instead of passing through to rustc's "expected type". (A float like
/// `.5` is a single Literal, not a `.` Punct, so it is not caught here.)
fn validate_range(tokens: &[TokenTree]) -> Option<Ty> {
    if tokens.iter().any(|t| matches!(t, TokenTree::Punct(p) if p.as_char() == '.')) {
        return Some(err_ty_at(
            "batch-impl: a range (`..`/`..=`) in a type position needs integer endpoints (e.g. `0..=3`)",
            span_at(tokens, 0),
        ));
    }
    None
}

/// `+`/`?`/`.` are only invalid at the *start* of a type (`dyn Trait + Send`
/// and `T: Clone + 'a` use them legally in the middle). A `.` only counts as
/// a lone punctuation when Alone — a Joint `.` heads `..`/`..=` (range
/// syntax). `!` (never) and `::` (absolute path) may be valid, left to rustc.
fn validate_start_punct(tokens: &[TokenTree]) -> Option<Ty> {
    if let Some(TokenTree::Punct(p)) = tokens.first()
        && (matches!(p.as_char(), '+' | '?')
            || (p.as_char() == '.' && p.spacing() == Spacing::Alone))
    {
        return Some(err_ty_at(
            "batch-impl: `+`/`?`/`.` is not valid at the start of a type \
             (`+`/`?` belong in bounds; a type cannot start with `.`)",
            p.span(),
        ));
    }
    None
}

/// Empty token node (fallback for unwrap_or_else)
pub(crate) fn empty() -> Ty {
    TyPrimitive(quote![]).to_ty()
}
