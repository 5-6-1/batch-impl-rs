//! Ident-based block families: `::` paths, macro calls, the fn family
//! (`fn` / `unsafe fn` / `extern "C" fn`), trait-object families
//! (`dyn ...` / `for ...` / `Fn ...` / `impl Trait`), the `impl{...}` /
//! `where{...}` attachment blocks, and the bare ident (a trait head when it
//! matches the annotated trait). Dispatched from
//! [`parse_block`](super::space::parse_block) via [`ident_block`].

use crate::ast::*;
use crate::parse::blocks::{cursor_is_arrow, peek_ident_at};
use crate::parse::generic::{empty, parse_angle_bracket_contents};
use crate::parse::parse_item;
use crate::parse::space::{parse_block, parse_return_expr, parse_return_expr_tokens, starts_block};
use crate::util::Cursor;
use proc_macro2::{Ident, TokenStream, TokenTree};
use quote::ToTokens;

/// Ident block: `::` paths (`std::vec::Vec`), macro calls (`m!(...)`), the
/// fn family (`fn` / `unsafe fn` / `extern "C" fn`), the trait-object
/// families (`dyn ...` / `for ...` / `Fn ...` / `impl Trait`), the
/// `impl{...}` / `where{...}` attachment blocks, or a bare ident (a trait
/// head when it matches the annotated trait).
pub(crate) fn ident_block(cursor: &mut Cursor, id: Ident, trait_name: Option<&Ident>) -> Ty {
    match id.to_string().as_str() {
        "fn" => fn_block(cursor, trait_name, false),
        "unsafe" if peek_ident_at(cursor, 1, "fn") => {
            cursor.bump(); // `unsafe`
            fn_block(cursor, trait_name, true)
        }
        "unsafe" => {
            // bare `unsafe` — unsafe impl marker (the chain attaches the target)
            cursor.bump();
            TyWithPrefix(TyPrefix::Unsafe, None).to_ty()
        }
        "self" => {
            cursor.bump();
            TyWithPrefix(TyPrefix::SelfType, None).to_ty()
        }
        "extern"
            if matches!(cursor.peek_at(1), Some(TokenTree::Literal(_)))
                && peek_ident_at(cursor, 2, "fn") =>
        {
            extern_fn_block(cursor)
        }
        "dyn" => dyn_block(cursor, trait_name),
        "for" => for_block(cursor, trait_name),
        "Fn" | "FnMut" | "FnOnce" | "AsyncFn" | "AsyncFnMut" | "AsyncFnOnce" => {
            // The Fn-family trait types (incl. the async closures of Rust
            // 2024) — structured like `fn`, so a bare `Fn` (params filled by
            // `.` later) and `Fn(A, B) -> R` both work. A path segment
            // (`Fn::assoc` — rare, but a qualified path starts with `::`)
            // must not be hijacked; `Fn` followed by `::` falls through to
            // the plain-ident path.
            if matches!(cursor.op_at(1), Some((crate::util::Op::ColonColon, _))) {
                plain_ident_block(cursor, id, trait_name)
            } else {
                let kind = match id.to_string().as_str() {
                    "FnMut" => FnKind::TraitMut,
                    "FnOnce" => FnKind::TraitOnce,
                    "AsyncFn" => FnKind::TraitAsync,
                    "AsyncFnMut" => FnKind::TraitAsyncMut,
                    "AsyncFnOnce" => FnKind::TraitAsyncOnce,
                    _ => FnKind::Trait,
                };
                fn_trait_block(cursor, &id, kind)
            }
        }
        "impl" => {
            // `impl{...}` shape template — the Brace group is the template
            if let Some(g) = cursor.peek_group_at(1, delimiter![{}]) {
                cursor.advance(2);
                TyWithImpl(None, TyImplTemplate(g.stream())).to_ty()
            } else {
                // bare `impl` — swallow the qualified type and `+` bounds
                swallow_chain(cursor, &id, trait_name)
            }
        }
        "where" => {
            // `where{...}` predicate suffix — the Brace group holds the predicates
            if let Some(g) = cursor.peek_group_at(1, delimiter![{}]) {
                cursor.advance(2);
                TyWithWhere(None, TyWhere(g.stream())).to_ty()
            } else {
                // bare `where` is not a type — plain ident path (errors downstream)
                plain_ident_block(cursor, id, trait_name)
            }
        }
        _ => plain_ident_block(cursor, id, trait_name),
    }
}

/// fn family: `fn` / `unsafe fn` — the parameter group is consumed, and an
/// optional `-> Ret` return type (a full space expression that stops at an
/// attachment block — `{...}` / `where{...}` / `impl{...}` belong to the
/// impl, not to the fn type). A bare `fn` keeps its params to be filled by
/// `.` later.
pub(crate) fn fn_block(cursor: &mut Cursor, trait_name: Option<&Ident>, is_unsafe: bool) -> Ty {
    cursor.bump(); // `fn`
    let params = if let Some(g) = cursor.peek_group(delimiter![()]) {
        cursor.bump();
        let args = g.stream().into_iter().collect::<Vec<_>>();
        let mut pc = Cursor::new(&args);
        let mut list = vec![];
        while let Some(p) = parse_item(&mut pc, Op::Comma, trait_name) {
            list.push(p);
        }
        Some(list)
    } else {
        None
    };
    let ret = if cursor_is_arrow(cursor) {
        cursor.advance(2);
        Some(parse_return_expr(cursor, trait_name))
    } else {
        None
    };
    TyFn(params, ret.map(Into::into), is_unsafe, FnKind::Bare).to_ty()
}

/// `extern "C" fn(...)` — one passthrough block (the ABI literal is not a
/// TyFn field).
pub(crate) fn extern_fn_block(cursor: &mut Cursor) -> Ty {
    // `extern` `"C"` `fn` — then the shared passthrough tail
    passthrough_block(cursor, 3)
}

/// `Fn(A) -> B` / `FnMut(A)` / `FnOnce(A)` — the Fn-family trait types.
/// Parsed structurally like `fn` (same `TyFn` shape, `FnKind` marks the
/// trait), so the `.().N` / `.().N..M` generators work on them — and a bare
/// `Fn` (no parens) keeps `None` params to be filled by `.` later, exactly
/// like a bare `fn`.
pub(crate) fn fn_trait_block(cursor: &mut Cursor, id: &Ident, kind: FnKind) -> Ty {
    cursor.bump(); // `Fn` / `FnMut` / `FnOnce`
    let params = if let Some(g) = cursor.peek_group(delimiter![()]) {
        cursor.bump();
        let args = g.stream().into_iter().collect::<Vec<_>>();
        let mut pc = Cursor::new(&args);
        let mut list = vec![];
        while let Some(p) = parse_item(&mut pc, Op::Comma, None) {
            list.push(p);
        }
        Some(list)
    } else {
        None
    };
    let ret = if cursor_is_arrow(cursor) {
        cursor.advance(2);
        Some(parse_return_expr(cursor, None))
    } else {
        None
    };
    let _ = id;
    TyFn(params, ret.map(Into::into), false, kind).to_ty()
}

/// Shared tail of the passthrough fn blocks (`extern "C" fn` / `Fn` /
/// `FnMut` / `FnOnce`): the already-bumped leading tokens, an optional
/// `(params)` group, and an optional `-> Ret` return expression are consumed
/// as one opaque token slice — the whole block is a passthrough.
fn passthrough_block(cursor: &mut Cursor, n_leading: usize) -> Ty {
    let start = cursor.pos();
    for _ in 0..n_leading {
        cursor.bump();
    }
    if matches!(cursor.peek(), Some(TokenTree::Group(g)) if g.delimiter() == delimiter![()]) {
        cursor.bump();
    }
    if cursor_is_arrow(cursor) {
        cursor.advance(2);
        // the return expression — consume its blocks without keeping them
        // structurally (the whole block is a passthrough)
        parse_return_expr_tokens(cursor);
    }
    let n = cursor.pos() - start;
    let tokens = cursor.slice_at(start, n).to_vec();
    TyPrimitive(tokens.into_iter().collect()).to_ty()
}

/// `for<'a> <inner>` — a higher-ranked trait bound. The binder (`<'a>`) is
/// kept verbatim; the qualified type is parsed **structurally** (so
/// `for<'a> Fn.().2` runs the Fn generator). Rendered back as
/// `for<'a> <inner>`.
pub(crate) fn for_block(cursor: &mut Cursor, trait_name: Option<&Ident>) -> Ty {
    cursor.bump(); // `for`
    let binder = if let Some(g) = cursor.peek_group(delimiter![<>]) {
        cursor.bump();
        g.stream()
    } else {
        quote::quote!()
    };
    let inner = crate::parse::chain::parse_dot_chain(cursor, trait_name).unwrap_or_else(empty);
    TyWithFor(binder, Box::new(inner)).to_ty()
}

/// `dyn ...` — a trait object. The qualified type after `dyn` is parsed
/// **structurally** (so `dyn Fn.().3` runs the Fn generator), and any
/// `+ Bound` tail rides along as token fragments. Rendered back as
/// `dyn <inner> + <bounds>`.
pub(crate) fn dyn_block(cursor: &mut Cursor, trait_name: Option<&Ident>) -> Ty {
    cursor.bump(); // `dyn`
    let inner = crate::parse::chain::parse_dot_chain(cursor, trait_name).unwrap_or_else(empty);
    let mut bounds = vec![];
    while cursor.is_punct('+') {
        let mut ts = cursor.peek().unwrap().to_token_stream();
        cursor.bump();
        if let Some(t) = cursor.peek()
            && (starts_block(t) || matches!(t, TokenTree::Punct(p) if p.as_char() == '+'))
        {
            let b = crate::parse::chain::parse_dot_chain(cursor, trait_name).unwrap_or_else(empty);
            ts.extend(b.to_token_stream());
        }
        bounds.push(ts);
    }
    TyWithDyn(Box::new(inner), bounds).to_ty()
}

/// `dyn ...` / `impl Trait` — swallow the qualified type and a `+ Bound`
/// chain (a block after the chain ends is the chain's next block).
pub(crate) fn swallow_chain(cursor: &mut Cursor, _id: &Ident, trait_name: Option<&Ident>) -> Ty {
    let start = cursor.pos();
    cursor.bump(); // `dyn` / `impl` — id is re-collected via the token slice
    parse_block(cursor, trait_name).unwrap_or_else(empty); // qualified type
    while cursor.is_punct('+') {
        cursor.bump();
        parse_block(cursor, trait_name).unwrap_or_else(empty);
    }
    let n = cursor.pos() - start;
    let tokens = cursor.slice_at(start, n).to_vec();
    TyPrimitive(tokens.into_iter().collect()).to_ty()
}

/// Plain ident: `::` path segments, a `!` macro call, a **trailing `<>`
/// argument group** (`Box<u8>` — the args belong to the ident, so `X Box<u8>`
/// applies the whole generic), or a bare ident (a trait head when it matches
/// the annotated trait).
pub(crate) fn plain_ident_block(cursor: &mut Cursor, id: Ident, trait_name: Option<&Ident>) -> Ty {
    let mut tokens = vec![TokenTree::Ident(id.clone())];
    cursor.bump();
    loop {
        match cursor.peek() {
            // `::` path segment stays in the block (read as one unit by the
            // operator dictionary); the segment ident is the third token.
            Some(TokenTree::Punct(_))
                if matches!(cursor.peek_op(), Some((crate::util::Op::ColonColon, _))) =>
            {
                let Some(TokenTree::Ident(seg)) = cursor.peek_at(2) else {
                    break;
                };
                let seg = seg.clone();
                tokens.push(cursor.peek().unwrap().clone());
                tokens.push(cursor.peek_at(1).unwrap().clone());
                tokens.push(TokenTree::Ident(seg));
                cursor.advance(3);
            }
            // `ident!(...)` macro call — passthrough
            Some(TokenTree::Punct(p))
                if p.as_char() == '!' && matches!(cursor.peek_at(1), Some(TokenTree::Group(_))) =>
            {
                tokens.push(TokenTree::Punct(p.clone()));
                tokens.push(cursor.peek_at(1).unwrap().clone());
                cursor.advance(2);
                break;
            }
            _ => break,
        }
    }
    // `Box<u8>` — a trailing `<>` group is the ident's argument list (the
    // args are consumed into the block, not a separate space application).
    if let Some(TokenTree::Group(g)) = cursor.peek()
        && g.delimiter() == delimiter![<>]
    {
        let args = g.stream().into_iter().collect::<Vec<_>>();
        cursor.bump();
        let base_tokens = tokens.into_iter().collect::<TokenStream>();
        let is_trait_head = matches!(base_tokens.clone().into_iter().next(),
            Some(TokenTree::Ident(i)) if trait_name.is_some_and(|tn| tn == &i));
        // Bindings/bounds in the args are only valid on a trait path
        // (`Conv<Item = u32> X`) or a generic declaration — a concrete
        // type's args are a plain type list.
        let params = parse_angle_bracket_contents(&args, trait_name, is_trait_head);
        return if is_trait_head {
            // trait head with args (`Tr<A>`) — apply turns it into the impl
            TyTrait(base_tokens, params).to_ty()
        } else {
            TyGeneric(TyPrimitive(base_tokens).to_ty().into(), params).to_ty()
        };
    }
    if let [TokenTree::Ident(single)] = tokens.as_slice()
        && trait_name.is_some_and(|t| t == single)
    {
        return TyTrait(
            TokenStream::from(TokenTree::Ident(single.clone())),
            TyTypeParam { params: vec![], bindings: vec![] },
        )
        .to_ty();
    }
    TyPrimitive(tokens.into_iter().collect()).to_ty()
}
