//! Block-family implementations for space-application parsing: each block
//! family (`&` refs, `*` pointers/splats, `@N` refs, numbers/ranges, idents,
//! the fn family, trait-object families) parses the smallest self-contained
//! type fragment plus its fixed suffixes. The dispatch lives in
//! [`parse_block`](super::space::parse_block); the helpers here are shared
//! with the space-chain skeleton in `space.rs`.
//!
//! A block never swallows the type it would apply to (`&mut u8` is the two
//! blocks `&mut` and `u8`, folded by the chain) — with two exceptions where
//! Rust syntax forces the fragment together: lifetime references
//! (`&'a mut u8`) and the fn family (`fn(u8) -> u8`).

use crate::apply::err_ty_at;
use crate::ast::fresh::at_ref_name;
use crate::ast::*;
use crate::parse::generic::{empty, split_at_depth0};
use crate::parse::parse_atom::parse_range;
use crate::parse::parse_item;
use crate::parse::space::parse_block;
use crate::util::Cursor;
use proc_macro2::{Delimiter, Ident, Spacing, TokenStream, TokenTree};
use quote::{ToTokens, quote};

/// Whether the cursor sits on a `->` fn arrow (Joint `-` followed by `>`).
pub(crate) fn cursor_is_arrow(cursor: &Cursor) -> bool {
    matches!(cursor.peek(), Some(TokenTree::Punct(p))
        if p.as_char() == '-' && p.spacing() == Spacing::Joint
            && matches!(cursor.peek_at(1), Some(TokenTree::Punct(q)) if q.as_char() == '>'))
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

/// Whether the cursor sits on a `'` + ident lifetime.
pub(crate) fn cursor_is_lifetime(cursor: &Cursor) -> bool {
    matches!(cursor.peek(), Some(TokenTree::Punct(p)) if p.as_char() == '\'')
        && matches!(cursor.peek_at(1), Some(TokenTree::Ident(_)))
}

/// `&` block family: `&` / `&mut` / `&'a` / `&'a mut` — the prefix never
/// swallows the target type (`&mut u8` = `&mut` + `u8`), except a lifetime
/// reference which is one block (`&'a mut u8` — a bare `&'a` is not a type).
pub(crate) fn reference_block(cursor: &mut Cursor, trait_name: Option<&Ident>) -> Ty {
    cursor.bump(); // `&`
    let mut is_mut = peek_ident_at(cursor, 0, "mut");
    if is_mut {
        cursor.bump();
    }
    let lifetime = if cursor_is_lifetime(cursor) {
        let lt = match cursor.peek_at(1) {
            Some(TokenTree::Ident(id)) => Ident::new(&id.to_string(), id.span()),
            _ => unreachable!(),
        };
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
        // render the whole reference as a passthrough.
        let ty = parse_block(cursor, trait_name).unwrap_or_else(empty);
        let mut ts =
            TokenStream::from(TokenTree::Punct(proc_macro2::Punct::new('&', Spacing::Alone)));
        ts.extend(lifetime_tokens(&lt));
        if is_mut {
            ts.extend(quote!(mut));
        }
        ts.extend(ty.to_token_stream());
        return TyPrimitive(ts).to_ty();
    }
    let prefix = if is_mut { TyPrefix::RefMut } else { TyPrefix::Ref };
    TyWithPrefix(prefix, None).to_ty()
}

/// `*` block family: `*const T` / `*mut T` prefixes (never swallow the
/// target: `*const u8` = `*const` + `u8`) and `*[...]` / `*(...)` splats
/// (one block each — the splat keeps its group).
pub(crate) fn star_block(cursor: &mut Cursor) -> Ty {
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
        Some(TokenTree::Group(g))
            if matches!(g.delimiter(), Delimiter::Bracket | Delimiter::Parenthesis) =>
        {
            let g = g.clone();
            cursor.bump();
            let inner = g.stream().into_iter().collect::<Vec<_>>();
            let elems = split_at_depth0(&inner, ',')
                .iter()
                .filter(|c| !c.is_empty())
                .map(|c| parse_item(&mut Cursor::new(c), Op::Space, None).unwrap_or_else(empty))
                .collect::<Vec<_>>();
            if g.delimiter() == Delimiter::Bracket {
                TySplat::Array(TyArray(elems)).to_ty()
            } else {
                TySplat::Tuple(TyTuple(elems)).to_ty()
            }
        }
        _ => err_ty_at(
            "batch-impl: `*` must be a splat (`*[...]` / `*(...)`) or a raw \
             pointer (`*const T` / `*mut T`)",
            cursor.span(),
        ),
    }
}

/// `@N` position reference (fresh-name resolution at the type-domain entry).
pub(crate) fn at_ref_block(cursor: &mut Cursor) -> Ty {
    let at_span = cursor.span();
    cursor.bump(); // `@`
    match cursor.peek() {
        Some(TokenTree::Literal(lit)) => match at_ref_name(&lit.to_string()) {
            Some(name) => {
                cursor.bump();
                if cursor.is_punct('.') {
                    return err_ty_at(
                        "batch-impl: `@N..M` range references are only \
                         allowed as a where-predicate subject \
                         (e.g. `where{@0..=2: Clone}`)",
                        at_span,
                    );
                }
                let ident = Ident::new(&name, at_span);
                TyPrimitive(quote!(#ident)).to_ty().with_span(at_span)
            }
            None => err_ty_at(
                "batch-impl: `@` in a type must be followed by a position \
                 digit (e.g. `@0` or `@0_1`)",
                at_span,
            ),
        },
        _ => err_ty_at(
            "batch-impl: `@` in a type must be a position digit (e.g. `@0` or `@0_1`)",
            at_span,
        ),
    }
}

/// Number / range block: `N` / `N..M` / `N..=M` (a range stays one block —
/// only the range's own tokens are examined, whatever follows is a chain
/// block).
pub(crate) fn literal_block(cursor: &mut Cursor) -> Ty {
    // `N..M` / `N..=M`
    if matches!(cursor.peek_at(1), Some(TokenTree::Punct(p)) if p.as_char() == '.')
        && matches!(cursor.peek_at(2), Some(TokenTree::Punct(q)) if q.as_char() == '.')
    {
        let n = if matches!(cursor.peek_at(3), Some(TokenTree::Punct(e)) if e.as_char() == '=') {
            5
        } else {
            4
        };
        let tokens = cursor.slice_at(cursor.pos(), n).to_vec();
        if let Some(range) = parse_range(&tokens) {
            cursor.advance(n);
            return range;
        }
        return err_ty_at(
            "batch-impl: a range (`..`/`..=`) in a type position needs integer \
             endpoints (e.g. `0..=3`)",
            cursor.span(),
        );
    }
    if let Some(TokenTree::Literal(lit)) = cursor.peek() {
        match lit.to_string().parse::<usize>() {
            Ok(number) => {
                cursor.bump();
                return TyNum(number).to_ty();
            }
            Err(_) => {
                return err_ty_at(
                    "batch-impl: a bare literal in a type position must be an \
                     integer (usize); float/string/char literals are not types",
                    lit.span(),
                );
            }
        }
    }
    err_ty_at("batch-impl: unexpected literal in a type position", cursor.span())
}
