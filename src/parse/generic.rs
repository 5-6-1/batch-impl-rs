//! Generic and angle-bracket parsing module.
//!
//! Provides matching and parsing of `<...>` generic parameters plus related helpers.

use proc_macro2::{Ident, Spacing, TokenStream, TokenTree};

use quote::quote;

use crate::apply::err_ty_at;
use crate::ast::*;
use crate::parse::parse_item;
use crate::parse::resolve_at_refs;
use crate::util::{Cursor, compile_error_ty, is_punct, is_single_colon, scan_stop};

// ============================================================
// Angle brackets and generic parameters
// ============================================================

/// Find the angle-bracket group after base (`delimiter![<>]`, produced by `angle_collect` pairing),
/// returning (base, args, rest). base must not be empty (empty = a type-parameter list,
/// handled by [`parse_type_params`]).
pub(crate) fn parse_generic(
    tokens: &[TokenTree],
) -> Option<(Vec<TokenTree>, TokenStream, Vec<TokenTree>)> {
    for (i, token) in tokens.iter().enumerate() {
        if let TokenTree::Group(g) = token
            && g.delimiter() == delimiter![<>]
        {
            if i == 0 {
                return None;
            }
            return Some((tokens[..i].to_vec(), g.stream(), tokens[i + 1..].to_vec()));
        }
    }
    None
}

/// Parse a bare type-parameter list that starts with an angle-bracket group (`<'a, T: Clone>`).
pub(crate) fn parse_type_params(tokens: &[TokenTree]) -> Option<(TokenStream, Vec<TokenTree>)> {
    let TokenTree::Group(g) = tokens.first()? else {
        return None;
    };
    if g.delimiter() != delimiter![<>] {
        return None;
    }
    Some((g.stream(), tokens[1..].to_vec()))
}

/// Whether base ends with trait_name's ident (distinguishes `TraitName<T>` from plain generics)
pub(crate) fn is_trait_base(base: &[TokenTree], trait_name: Option<&Ident>) -> bool {
    trait_name
        .is_some_and(|name| matches!(base.last(), Some(TokenTree::Ident(last)) if last == name))
}

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
        chunks.push(&rest[..index]);
        rest = &rest[index + 1..];
    }
    chunks.push(rest);
    chunks
}

/// Find the first `:` that is not part of `::` (used to split `T: Bound`)
fn find_colon_at_depth0(tokens: &[TokenTree]) -> Option<usize> {
    scan_stop(tokens, &[':']).filter(|&index| is_single_colon(tokens, index))
}

/// Parse `<T: Clone, U, Item=V>` contents: parameter list + associated-type bindings
pub(crate) fn parse_angle_bracket_contents(
    tokens: &[TokenTree], trait_name: Option<&Ident>, allow_special: bool,
) -> TyTypeParam {
    // `allow_special`: bindings (`Item = u32`) and bounds (`T: Clone`) are
    // valid only on a trait path (`Conv<Item = u32> X`) or in a generic
    // declaration (`<T: Clone> Foo`) — a concrete type's args are a plain
    // type list, so `=`/`:` there is a usage error (previously the bound was
    // silently dropped and a struct binding rendered invalid code).
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
            if allow_special {
                let name_ty = TyPrimitive(chunk[..eq].iter().cloned().collect()).to_ty();
                let value = match resolve_at_refs(&chunk[eq + 1..]) {
                    Ok(v) if v.is_empty() => TyPrimitive(compile_error_ty(
                        "batch-impl: binding `Item =` missing a value (write `Item = u32`)",
                        chunk[eq].span(),
                    ))
                    .to_ty(),
                    Ok(v) => {
                        parse_item(&mut Cursor::new(&v), Op::Dash, trait_name).unwrap_or_else(empty)
                    }
                    Err(e) => TyPrimitive(e).to_ty(),
                };
                bindings.push((Box::new(name_ty), Box::new(value)));
            } else {
                params.push((
                    Box::new(
                        TyPrimitive(compile_error_ty(
                            "batch-impl: binding args (`Item = u32`) are only valid on a trait path (`Conv<Item = u32> X`) or in a generic declaration — a concrete type's args are a plain type list",
                            chunk[eq].span(),
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
                        TyPrimitive(chunk[..colon].iter().cloned().collect::<TokenStream>())
                            .to_ty(),
                    ),
                    Some(if chunk[colon + 1..].is_empty() {
                        TyPrimitive(compile_error_ty(
                            "batch-impl: bound `T:` missing a bound (write `T: Clone`)",
                            chunk[colon].span(),
                        ))
                        .to_ty()
                    } else {
                        parse_item(&mut Cursor::new(&chunk[colon + 1..]), Op::Dash, trait_name)
                            .unwrap_or_else(empty)
                    }),
                ));
            } else {
                params.push((
                    Box::new(
                        TyPrimitive(compile_error_ty(
                            "batch-impl: bound args (`T: Clone`) are only valid on a trait path or in a generic declaration (`<T: Clone> Foo`) — a concrete type's args are a plain type list",
                            chunk[colon].span(),
                        ))
                        .to_ty(),
                    ),
                    None,
                ));
            }
        } else {
            let name = if matches!(
                chunk.first(),
                Some(TokenTree::Punct(p)) if p.as_char() == '@'
            ) && matches!(chunk.get(1), Some(TokenTree::Literal(_)))
                && chunk.iter().any(|t| matches!(t, TokenTree::Punct(p) if p.as_char() == '.'))
            {
                // `@N..M` range refs are where-predicate-only — reject them in
                // args with a targeted message instead of leaking raw tokens.
                let span = chunk[0].span();
                TyPrimitive(compile_error_ty(
                    "batch-impl: `@N..M` range references are only valid as a where-predicate subject",
                    span,
                ))
                .to_ty()
            } else {
                match resolve_at_refs(chunk) {
                    Ok(v) => {
                        parse_item(&mut Cursor::new(&v), Op::Dash, trait_name).unwrap_or_else(empty)
                    }
                    Err(e) => TyPrimitive(e).to_ty(),
                }
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
    if let Some(e) = validate_adjacent(tokens) {
        return e;
    }
    TyPrimitive(tokens.iter().cloned().collect()).to_ty().with_span(span)
}

/// `;`/`=`/`@`/`#` at depth 0 in a type position are always invalid — `;` is
/// the `batch_trait!` segment boundary; `=`/`@`/`#` have no legal role in a
/// type (they belong inside `<...>` or before parsing). The `=` of `..=` is
/// part of the range operator, not a binding, so a leftover after an earlier
/// error must not cascade a second diagnostic.
fn validate_stray_punct(tokens: &[TokenTree]) -> Option<Ty> {
    for (i, tt) in tokens.iter().enumerate() {
        if let TokenTree::Punct(p) = tt {
            let is_range_inclusive = p.as_char() == '=' && i > 0 && is_punct(&tokens[i - 1], '.');
            let msg = if is_range_inclusive {
                None
            } else {
                match p.as_char() {
                    ';' => Some(
                        "batch-impl: `;` is not valid in a type (it is the `batch_trait!` \
                         segment boundary; in `#[batch_impl]` specs are separated by `,`)",
                    ),
                    '=' => Some(
                        "batch-impl: `=` is not valid in a type position (associated-type \
                         bindings like `Item = u32` belong inside a trait path's `<...>`)",
                    ),
                    '@' => Some(
                        "batch-impl: `@` inside a type (position references like `@0` must \
                         start an operand, e.g. `T.@0`)",
                    ),
                    '#' => Some(
                        "batch-impl: `#` inside a type (attributes belong at the spec start \
                         as `#[...].T`; directives are expanded before parsing)",
                    ),
                    _ => None,
                }
            };
            if let Some(msg) = msg {
                return Some(err_ty_at(msg, p.span()));
            }
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
            tokens[0].span(),
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

/// Adjacent type fragments without an operator (`A B`, `Vec<T>U`, `[A B]`)
/// would otherwise render as invalid Rust with no guidance. Only true adjacent
/// Ident/Literal/Group pairs trigger; paths (`a::b`) and ranges (`0..3`) carry
/// a punct between the fragments, and generic application (`Vec<u32>`) is an
/// Ident directly followed by a `<>`/`()` group — all excluded. Lifetime names
/// (`'a` tokenizes as `'` + `a`) and trait-object heads (`dyn`) are not type
/// fragments.
fn validate_adjacent(tokens: &[TokenTree]) -> Option<Ty> {
    let mut prev: Option<&TokenTree> = None;
    let mut prev_is_lifetime_name = false;
    for tt in tokens {
        let is_lifetime_name = matches!(prev, Some(TokenTree::Punct(p)) if p.as_char() == '\'');
        let prev_is_fragment = prev.is_some_and(|p| {
            matches!(p, TokenTree::Ident(_) | TokenTree::Literal(_) | TokenTree::Group(_))
        }) && !prev_is_lifetime_name;
        let is_fragment = !is_lifetime_name
            && matches!(tt, TokenTree::Ident(_) | TokenTree::Literal(_) | TokenTree::Group(_));
        // `fn`/`unsafe`/`dyn` may head an fn type or trait object
        // (`fn(A)->B` / `unsafe fn(A)` / `dyn Trait + Send`) — parse_function
        // and the dyn path own those; reaching primitive is an anomaly, not
        // two adjacent types.
        let fn_head = matches!(tt, TokenTree::Ident(id) if id == "fn" || id == "unsafe" || id == "dyn")
            || matches!(prev, Some(TokenTree::Ident(id)) if id == "fn" || id == "unsafe" || id == "dyn");
        // `Vec<u32>` / `Fn(A)` — an Ident immediately followed by a `<>` group
        // (an angle-collected `Delimiter::None` group) or a `()` group is
        // generic application / trait-object call syntax, not adjacency.
        let after_ident = matches!(prev, Some(TokenTree::Ident(_)));
        let group_is_generic_or_call = matches!(
            tt,
            TokenTree::Group(g)
                if g.delimiter() == proc_macro2::Delimiter::None
                    || g.delimiter() == proc_macro2::Delimiter::Parenthesis
        );
        let generic_app = after_ident && group_is_generic_or_call;
        if prev_is_fragment && is_fragment && !fn_head && !generic_app {
            return Some(err_ty_at(
                "batch-impl: adjacent types without an operator (missing `.` / `-` / `,`)",
                tt.span(),
            ));
        }
        prev = Some(tt);
        prev_is_lifetime_name = is_lifetime_name;
    }
    None
}

/// Empty token node (fallback for unwrap_or_else)
pub(crate) fn empty() -> Ty {
    TyPrimitive(quote![]).to_ty()
}
