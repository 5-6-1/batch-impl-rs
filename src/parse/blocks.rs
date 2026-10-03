//! Block-family implementations for space-application parsing: each block
//! family (`&` refs, `*` pointers/packs, `@N` refs, numbers/ranges, idents,
//! the fn family, trait-object families) parses the smallest self-contained
//! type fragment plus its fixed suffixes. The dispatch lives in
//! [`parse_block`](super::space::parse_block); the helpers here are shared
//! with the space-chain skeleton in `space.rs`.
//!
//! A block never swallows the type it would apply to (`&mut u8` is the two
//! blocks `&mut` and `u8`, folded by the chain), except for lifetime references
//! (`&'a mut u8`), the fn family (`fn(u8) -> u8`) and the Pack prefix `*T`,
//! which consumes exactly one following block.

use crate::apply::Star;
use crate::apply::err_ty_at;
use crate::ast::*;
use crate::parse::Ctx;
use crate::parse::generic::empty;
use crate::parse::parse_atom::parse_range;
use crate::parse::space::parse_block;
use crate::util::Cursor;
use proc_macro2::{Ident, Spacing, TokenStream, TokenTree};
use quote::quote;

/// Whether the cursor sits on a `->` fn arrow (Joint `-` followed by `>`).
pub(crate) fn cursor_is_arrow(cursor: &Cursor) -> bool {
    matches!(cursor.peek_op(), Some((crate::util::Op::Arrow, _)))
}

/// `'a` lifetime tokens (`'` punct + ident).
pub(crate) fn lifetime_tokens(lt: &Ident) -> TokenStream {
    let mut ts = TokenStream::from(TokenTree::Punct(proc_macro2::Punct::new('\'', Spacing::Joint)));
    ts.extend(TokenStream::from(TokenTree::Ident(lt.clone())));
    ts
}

/// Whether `tokens[off]` is an ident equal to `name`.
pub(crate) fn peek_ident_at(cursor: &Cursor, off: usize, name: &str) -> bool {
    matches!(cursor.peek_at(off), Some(TokenTree::Ident(id)) if id == name)
}

/// Whether the cursor sits on a `'` + ident lifetime; returns the lifetime
/// ident when present (one test + extraction — callers use the ident
/// directly instead of re-peeking).
pub(crate) fn cursor_lifetime(cursor: &Cursor) -> Option<Ident> {
    let id = match cursor.peek_at(1) {
        Some(TokenTree::Ident(id))
            if matches!(cursor.peek(),
            Some(TokenTree::Punct(p)) if p.as_char() == '\'') =>
        {
            id
        }
        _ => return None,
    };
    Some(Ident::new(&id.to_string(), id.span()))
}

/// `&` block family: `&` / `&mut` / `&'a` / `&'a mut` — the prefix never
/// swallows the target type (`&mut u8` = `&mut` + `u8`), except a lifetime
/// reference which is one block (`&'a mut u8` — a bare `&'a` is not a type).
pub(crate) fn reference_block(cursor: &mut Cursor, ctx: Ctx<'_>) -> Ty {
    cursor.bump(); // `&`
    let mut is_mut = peek_ident_at(cursor, 0, "mut");
    if is_mut {
        cursor.bump();
    }
    let lifetime = if let Some(lt) = cursor_lifetime(cursor) {
        cursor.advance(2);
        if peek_ident_at(cursor, 0, "mut") {
            is_mut = true;
            cursor.bump();
        }
        Some(lt)
    } else {
        None
    };
    if let Some(lt) = lifetime {
        // `&'a u8` / `&'a mut u8` — one block: swallow the target type and
        // keep the target structural inside a fixed Rust prefix. It is a
        // **sub-type position** (`T: &'a Vec<Item = u8>`), so the bound flag
        // stops here.
        let ty = parse_block(cursor, ctx.plain()).unwrap_or_else(empty);
        let mut ts =
            TokenStream::from(TokenTree::Punct(proc_macro2::Punct::new('&', Spacing::Alone)));
        ts.extend(lifetime_tokens(&lt));
        if is_mut {
            ts.extend(quote!(mut));
        }
        return TyPrefixed(ts, ty.into()).to_ty();
    }
    let prefix = if is_mut { TyPrefix::RefMut } else { TyPrefix::Ref };
    TyWithPrefix(prefix, None).to_ty()
}

/// `*` block family: `*const T` / `*mut T` prefixes (never swallow the
/// target: `*const u8` = `*const` + `u8`) and `*T` packs. A pack consumes
/// exactly one block: `*Vec u8` maps `Vec` over `u8`, while `*(Vec u8)`
/// packs the grouped application result.
pub(crate) fn star_block(cursor: &mut Cursor, ctx: Ctx<'_>) -> Ty {
    let span = cursor.span();
    cursor.bump(); // `*`
    match cursor.peek() {
        Some(TokenTree::Ident(id)) if id == "const" => {
            cursor.bump();
            TyWithPrefix(TyPrefix::PtrConst, None).to_ty()
        }
        Some(TokenTree::Ident(id)) if id == "mut" => {
            cursor.bump();
            TyWithPrefix(TyPrefix::PtrMut, None).to_ty()
        }
        _ => match parse_block(cursor, ctx) {
            Some(ty) => ty.star().with_span(span),
            None => err_ty_at(
                "batch-impl: `*` needs a type block (write `*T` or `*[A, B]`); \
                 raw pointers use `*const T` or `*mut T`",
                span,
            ),
        },
    }
}

/// `@N` position reference (fresh-name resolution at the type-domain entry);
/// `@N..` / `@N..M` / `@L_N..` range references become a **structured**
/// [`TyKind::Fresh`] node — a leaf that rides the Ty tree and renders back to
/// the self-delimiting `@{...}` carrier for the token-level resolvers. No
/// reserved placeholder ident is minted. Also accepts the folded carrier
/// form (`@ { ... }`) directly, so output of an earlier fold re-parses.
pub(crate) fn at_ref_block(cursor: &mut Cursor) -> Ty {
    let at_span = cursor.span();
    cursor.bump(); // `@`
    match cursor.peek() {
        // Folded carrier: `@{...}` — parse the group's inner spelling.
        Some(TokenTree::Group(g)) if g.delimiter() == delimiter![{}] => {
            // The single authority for the carrier's inner spelling
            // (`carrier_inner` — the token-to-string join must not be
            // re-derived; this branch folds a carrier that earlier passes
            // emitted, so the same join keeps the round-trip exact).
            let inner = crate::ast::fresh_protocol::carrier_inner(g);
            if let Some(r) = FreshRef::parse(&inner) {
                cursor.bump();
                return TyFresh(r).to_ty().with_span(at_span);
            }
            AtRefError::position_reference(at_span).into_ty()
        }
        Some(TokenTree::Literal(lit)) => {
            let lit_str = lit.to_string();
            // `@N..` / `@L_N..` / `@N..M` / `@N..=M`: a structured range ref.
            let range_lit = crate::parse::parse_range_literal(&lit_str);
            if let Some((group, start)) = range_lit
                && let Some((op, _)) = cursor.op_at(1)
                && matches!(op, crate::util::Op::DotDot | crate::util::Op::DotDotEq)
            {
                let inclusive = matches!(op, crate::util::Op::DotDotEq);
                cursor.bump(); // the literal
                cursor.bump(); // first `.`
                cursor.bump(); // second `.`
                if inclusive {
                    cursor.bump(); // `=`
                }
                // The closed end: `@N..=M` keeps M, `@N..M` (exclusive)
                // normalizes to `..=M-1` — matching the where-predicate
                // resolution (`FreshRef::Closed` is always inclusive).
                let end = if let Some(TokenTree::Literal(el)) = cursor.peek() {
                    let Some(e) = el.to_string().parse::<usize>().ok() else {
                        return AtRefError::range_end_not_a_number(at_span).into_ty();
                    };
                    cursor.bump();
                    if inclusive || start < e {
                        FreshEnd::Closed(if inclusive { e } else { e - 1 })
                    } else {
                        // empty exclusive range (`@2..1`) — a typo; the closed
                        // form cannot represent it. The message names the
                        // numbers (F1 of the second review round).
                        return AtRefError::empty_exclusive_range(start, e, at_span).into_ty();
                    }
                } else {
                    FreshEnd::Open
                };
                return TyFresh(FreshRef { group, start, end }).to_ty().with_span(at_span);
            }
            // `@N` / `@g_i`: a single-position reference.
            match parse_single_ref(&lit_str) {
                Some(fresh) => {
                    cursor.bump();
                    TyFresh(fresh).to_ty().with_span(at_span)
                }
                None => AtRefError::position_digit(at_span).into_ty(),
            }
        }
        _ => AtRefError::not_a_position_digit(at_span).into_ty(),
    }
}

/// Parses a single-position reference literal: `N` → flat, `g_i` → grouped.
fn parse_single_ref(lit: &str) -> Option<FreshRef> {
    use crate::ast::fresh_protocol::{FreshEnd, FreshRef};
    if let Ok(n) = lit.parse::<usize>() {
        return Some(FreshRef { group: None, start: n, end: FreshEnd::Single });
    }
    let (l, i) = lit.split_once('_')?;
    Some(FreshRef { group: Some(l.parse().ok()?), start: i.parse().ok()?, end: FreshEnd::Single })
}

/// Number / range block: `N` / `N..M` / `N..=M` (a range stays one block —
/// only the range's own tokens are examined, whatever follows is a chain
/// block).
pub(crate) fn literal_block(cursor: &mut Cursor) -> Ty {
    // Every path — including the error paths — must **consume its tokens**: an
    // error node that leaves the cursor in place makes the enclosing
    // space-application chain fold the same token forever, and the depth cap
    // then reported "space-application chain exceeds 129 levels" instead of the
    // literal/range message below (measured: `#[batch_impl(1.5)]` and
    // `#[batch_impl(1..x)]` both reported the depth guard).
    // `N..M` / `N..=M` — the range operator read off the dictionary
    let n = match cursor.op_at(1) {
        Some((crate::util::Op::DotDot, _)) => 4,
        Some((crate::util::Op::DotDotEq, _)) => 5,
        _ => {
            // a bare number
            return match cursor.peek() {
                Some(TokenTree::Literal(lit)) => match lit.to_string().parse::<usize>() {
                    Ok(number) => {
                        cursor.bump();
                        TyNum(number).to_ty()
                    }
                    Err(_) => {
                        let span = lit.span();
                        cursor.bump();
                        err_ty_at(
                            "batch-impl: a bare literal in a type position must be an \
                             integer (usize); float/string/char literals are not types",
                            span,
                        )
                    }
                },
                _ => err_ty_at("batch-impl: unexpected literal in a type position", cursor.span()),
            };
        }
    };
    let tokens = cursor.slice_at(cursor.pos(), n).to_vec();
    if let Some(range) = parse_range(&tokens) {
        cursor.advance(n);
        return range;
    }
    let span = cursor.span();
    cursor.advance(n);
    err_ty_at(
        "batch-impl: a range (`..`/`..=`) in a type position needs integer \
         endpoints (e.g. `0..=3`)",
        span,
    )
}
