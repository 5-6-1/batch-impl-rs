//! Ident-based block families: paths (plain and leading-`::` **global**), macro
//! calls, the fn family (`fn` / `unsafe fn` / `extern "C" fn`, incl. named
//! parameters), trait-object families (`dyn ...` / `for ...` / `Fn ...` /
//! `impl Trait`), the `impl{...}` / `where{...}` attachment blocks, and the bare
//! ident (a trait head when it matches the annotated trait). Dispatched from
//! [`parse_block`](super::space::parse_block) via [`ident_block`].

use crate::apply::err_ty_at;
use crate::ast::*;
use crate::parse::blocks::{cursor_is_arrow, peek_ident_at};
use crate::parse::generic::{empty, parse_angle_bracket_contents};
use crate::parse::space::{parse_block, parse_return_expr, starts_block};
use crate::parse::{Ctx, parse_item};
use crate::util::Cursor;
use proc_macro2::{Group, Ident, TokenStream, TokenTree};
use quote::quote;

/// The parameter-list style of a `fn(...)` / `Fn(...)` type. A `fn(...)`
/// **pointer** type accepts named parameters (`fn(x: u8) -> u8` is valid
/// Rust); the `Fn(...)` sugar does not ("`Trait(...)` syntax does not support
/// named parameters"), so the two share this parser and the style decides
/// whether a `name:` prefix is consumed or reported.
#[derive(Copy, Clone, PartialEq, Eq)]
enum ParamStyle {
    /// `fn(...)` — named parameters are legal.
    Bare,
    /// `Fn(...)` / `FnMut(...)` / ... — named parameters are a user error.
    Sugar,
}

/// Ident block: `::` paths (`std::vec::Vec`), macro calls (`m!(...)`), the
/// fn family (`fn` / `unsafe fn` / `extern "C" fn`), the trait-object
/// families (`dyn ...` / `for ...` / `Fn ...` / `impl Trait`), the
/// `impl{...}` / `where{...}` attachment blocks, or a bare ident (a trait
/// head when it matches the annotated trait).
pub(crate) fn ident_block(cursor: &mut Cursor, id: Ident, ctx: Ctx<'_>) -> Ty {
    match id.to_string().as_str() {
        "fn" => fn_block(cursor, ctx, false),
        "unsafe" if peek_ident_at(cursor, 1, "fn") => {
            cursor.bump(); // `unsafe`
            fn_block(cursor, ctx, true)
        }
        "unsafe" if is_extern_fn(cursor, 1) => extern_fn_block(cursor, ctx, true),
        "unsafe" => {
            // bare `unsafe` — unsafe impl marker (the chain attaches the target)
            cursor.bump();
            TyWithPrefix(TyPrefix::Unsafe, None).to_ty()
        }
        "self" => {
            cursor.bump();
            TyWithPrefix(TyPrefix::SelfType, None).to_ty()
        }
        "extern" if is_extern_fn(cursor, 0) => extern_fn_block(cursor, ctx, false),
        "dyn" => dyn_block(cursor, ctx),
        "for" => for_block(cursor, ctx),
        "Fn" | "FnMut" | "FnOnce" | "AsyncFn" | "AsyncFnMut" | "AsyncFnOnce" => {
            // The Fn-family trait types (incl. the async closures of Rust
            // 2024) — structured like `fn`, so a bare `Fn` (params filled by
            // `.` later) and `Fn(A, B) -> R` both work. A path segment
            // (`Fn::assoc` — rare, but a qualified path starts with `::`)
            // must not be hijacked; `Fn` followed by `::` falls through to
            // the plain-ident path.
            if matches!(cursor.op_at(1), Some((crate::util::Op::ColonColon, _))) {
                plain_ident_block(cursor, id, ctx)
            } else {
                let kind = match id.to_string().as_str() {
                    "FnMut" => FnKind::TraitMut,
                    "FnOnce" => FnKind::TraitOnce,
                    "AsyncFn" => FnKind::TraitAsync,
                    "AsyncFnMut" => FnKind::TraitAsyncMut,
                    "AsyncFnOnce" => FnKind::TraitAsyncOnce,
                    _ => FnKind::Trait,
                };
                fn_trait_block(cursor, kind, ctx)
            }
        }
        "impl" => {
            // `impl{...}` shape template — the Brace group is the template
            if let Some(g) = cursor.peek_group_at(1, delimiter![{}]) {
                cursor.advance(2);
                TyWithImpl(None, TyImplTemplate(g.stream())).to_ty()
            } else {
                // `impl Trait` has a structured bound behind its fixed prefix.
                impl_trait_block(cursor, ctx)
            }
        }
        "where" => {
            // `where{...}` predicate suffix — the Brace group holds the predicates
            if let Some(g) = cursor.peek_group_at(1, delimiter![{}]) {
                cursor.advance(2);
                TyWithWhere(None, TyWhere(g.stream())).to_ty()
            } else {
                // bare `where` is not a type — plain ident path (errors downstream)
                plain_ident_block(cursor, id, ctx)
            }
        }
        _ => plain_ident_block(cursor, id, ctx),
    }
}

/// fn family: `fn` / `unsafe fn` — the parameter group is consumed, and an
/// optional `-> Ret` return type (a full space expression that stops at an
/// attachment block — `{...}` / `where{...}` / `impl{...}` belong to the
/// impl, not to the fn type). A bare `fn` keeps its params to be filled by
/// `.` later.
///
/// The parameter and return types are **sub-type positions**: a `fn(...)`
/// reached inside a bound (`T: fn(Vec<Item = u8>)`) must not let the bound flag
/// reach them, so `ctx.plain()` is passed down (a `dyn` / `for<'a>` inside them
/// re-enters a bound on its own).
pub(crate) fn fn_block(cursor: &mut Cursor, ctx: Ctx<'_>, is_unsafe: bool) -> Ty {
    let ctx = ctx.plain();
    cursor.bump(); // `fn`
    let params = cursor.peek_group(delimiter![()]).map(|g| {
        cursor.bump();
        parse_fn_params(&g, ctx, ParamStyle::Bare)
    });
    let ret = if cursor_is_arrow(cursor) {
        cursor.advance(2);
        Some(parse_return_expr(cursor, ctx))
    } else {
        None
    };
    finish_fn_type(params, ret, is_unsafe, FnKind::Bare)
}

/// A failed return expression must remain the chain's error. Hiding it in a
/// completed fn would let a remaining follower replace it with "already has a
/// return type", losing the original diagnostic and its offending token span.
fn finish_fn_type(params: Option<Vec<Ty>>, ret: Option<Ty>, is_unsafe: bool, kind: FnKind) -> Ty {
    match ret {
        Some(error) if matches!(error.kind, TyKind::Error(_)) => error,
        ret => TyFn(params, ret.map(Into::into), is_unsafe, kind).to_ty(),
    }
}

/// One `fn(...)` / `Fn(...)` parameter list: type operands separated by
/// commas, with the `name:` prefix of a **named** fn parameter handled
/// according to `style` (see [`ParamStyle`]).
fn parse_fn_params(g: &Group, ctx: Ctx<'_>, style: ParamStyle) -> Vec<Ty> {
    let args = g.stream().into_iter().collect::<Vec<_>>();
    let mut pc = Cursor::new(&args);
    let mut list = vec![];
    while let Some(p) = parse_fn_param(&mut pc, ctx, style) {
        list.push(p);
    }
    list
}

/// One parameter of a `fn(...)` / `Fn(...)` list. The separator stays in the
/// cursor between operands (the `parse_item(Op::Comma)` contract), so a named
/// parameter consumes its type up to — not including — the following comma.
fn parse_fn_param(cursor: &mut Cursor, ctx: Ctx<'_>, style: ParamStyle) -> Option<Ty> {
    // ABI function pointers can end in a C variadic slot. Keep this Rust
    // marker opaque instead of trying to parse its dots as DSL application.
    let skip_comma = usize::from(cursor.is_punct(','));
    if style == ParamStyle::Bare
        && (0..3).all(|i| {
            matches!(cursor.peek_at(skip_comma + i), Some(TokenTree::Punct(p)) if p.as_char() == '.')
        })
        && (cursor.peek_at(skip_comma + 3).is_none()
            || matches!(cursor.peek_at(skip_comma + 3), Some(TokenTree::Punct(p)) if p.as_char() == ','))
    {
        cursor.advance(skip_comma + 3);
        return Some(TyPrimitive(quote!(...)).to_ty());
    }
    // Rust parameter attributes precede its optional name. Keep them opaque
    // with the name while parsing only the type as DSL. Other commas stay
    // with `parse_item`, which owns the consecutive-comma diagnostic.
    let mut name_offset = skip_comma;
    while matches!(cursor.peek_at(name_offset), Some(TokenTree::Punct(p)) if p.as_char() == '#')
        && cursor.peek_group_at(name_offset + 1, delimiter![[]]).is_some()
    {
        name_offset += 2;
    }
    if let (Some(TokenTree::Ident(id)), Some((crate::util::Op::Colon, _))) =
        (cursor.peek_at(name_offset), cursor.op_at(name_offset + 1))
    {
        let name = id.clone();
        let span = id.span();
        let attrs: TokenStream = cursor
            .slice_at(cursor.pos() + skip_comma, name_offset - skip_comma)
            .iter()
            .cloned()
            .collect();
        // Both arms consume the name, its `:` and the parameter's tokens — the
        // loop above trusts `Some` to mean progress (a diagnostic that leaves
        // the cursor in place would spin until memory dies).
        cursor.advance(name_offset + 2);
        let segment = cursor.take_segment(&[',']);
        if style == ParamStyle::Sugar {
            return Some(err_ty_at(
                "batch-impl: the `Fn(…)` trait sugar does not support named parameters \
                 (`Fn(x: u8)`) — remove the name (a named parameter is only valid in a \
                 `fn(x: u8)` pointer type)",
                span,
            ));
        }
        if segment.is_empty() {
            return Some(err_ty_at(
                &format!(
                    "batch-impl: named parameter `{name}:` is missing a type (write `{name}: u8`)"
                ),
                span,
            ));
        }
        // The name is Rust-only syntax the DSL type grammar has no room for, so
        // it is kept verbatim as a prefix and the type is parsed structurally —
        // DSL operands keep working inside a named parameter (`fn(x: Box<u8>)`).
        let ty = parse_item(&mut Cursor::new(segment), Op::Space, ctx).unwrap_or_else(empty);
        return Some(TyPrefixed(quote!(#attrs #name :), ty.into()).to_ty());
    }
    parse_item(cursor, Op::Comma, ctx)
}

/// `extern "C" fn(...)` — the fixed ABI prefix wraps a structured function
/// type, so parameter/return packs reach their respective slot consumers.
fn extern_fn_block(cursor: &mut Cursor, ctx: Ctx<'_>, is_unsafe: bool) -> Ty {
    let mut leading = usize::from(is_unsafe) + 1; // optional `unsafe`, then `extern`
    if matches!(cursor.peek_at(leading), Some(TokenTree::Literal(_))) {
        leading += 1; // optional ABI string
    }
    let prefix = cursor.slice_at(cursor.pos(), leading).iter().cloned().collect();
    cursor.advance(leading);
    TyPrefixed(prefix, fn_block(cursor, ctx, false).into()).to_ty()
}

/// `unsafe extern ... fn` is one function type, whereas a bare `unsafe`
/// remains the impl marker. Rust also permits an omitted ABI literal.
fn is_extern_fn(cursor: &Cursor, offset: usize) -> bool {
    peek_ident_at(cursor, offset, "extern")
        && (peek_ident_at(cursor, offset + 1, "fn")
            || (matches!(cursor.peek_at(offset + 1), Some(TokenTree::Literal(_)))
                && peek_ident_at(cursor, offset + 2, "fn")))
}

/// `Fn(A) -> B` / `FnMut(A)` / `FnOnce(A)` — the Fn-family trait types.
/// Parsed structurally like `fn` (same `TyFn` shape, `FnKind` marks the
/// trait), so the `.().N` / `.().N..M` generators work on them — and a bare
/// `Fn` (no parens) keeps `None` params to be filled by `.` later, exactly
/// like a bare `fn`.
pub(crate) fn fn_trait_block(cursor: &mut Cursor, kind: FnKind, ctx: Ctx<'_>) -> Ty {
    let ctx = ctx.plain();
    cursor.bump(); // `Fn` / `FnMut` / `FnOnce`
    let params = cursor.peek_group(delimiter![()]).map(|g| {
        cursor.bump();
        // The `Fn(…)` sugar rejects named parameters (`ParamStyle::Sugar`) —
        // rustc's own rule for `Trait(...)` syntax.
        parse_fn_params(&g, ctx, ParamStyle::Sugar)
    });
    let ret = if cursor_is_arrow(cursor) {
        cursor.advance(2);
        Some(parse_return_expr(cursor, ctx))
    } else {
        None
    };
    finish_fn_type(params, ret, false, kind)
}

/// `for<'a, 'b> <inner>` — a higher-ranked trait bound. The **binder is parsed**
/// as a list of lifetimes (each may carry its `: 'b` bound tokens), so `for<u8>`
/// — a type parameter, which Rust declares on the impl, not in a binder — is
/// reported here instead of reaching rustc; the inner type is parsed
/// **structurally** (so `for<'a> Fn.().2` runs the Fn generator). Rendered back as
/// `for<'a, 'b> <inner>`. The binder governs a **bound** (Rust's grammar), so
/// `for<'a> Iterator<Item = u8>` takes its bindings.
pub(crate) fn for_block(cursor: &mut Cursor, ctx: Ctx<'_>) -> Ty {
    cursor.bump(); // `for`
    let mut binder = vec![];
    if let Some(g) = cursor.peek_group(delimiter![<>]) {
        cursor.bump();
        let args = g.stream().into_iter().collect::<Vec<_>>();
        for chunk in crate::parse::generic::split_at_depth0(&args, ',') {
            if chunk.is_empty() {
                continue;
            }
            // A binder element starts with `'` + ident (a lifetime); anything
            // else is a declaration in the wrong place, reported rather than
            // passed through for rustc to puzzle over.
            let is_lifetime =
                matches!(chunk.first(), Some(TokenTree::Punct(p)) if p.as_char() == '\'');
            if !is_lifetime {
                let span = chunk.first().map_or_else(proc_macro2::Span::call_site, |t| t.span());
                binder.push(err_ty_at(
                    "batch-impl: a `for<…>` binder holds lifetimes (`for<'a>`) — a type or \
                     const parameter is declared on the impl, not in the binder",
                    span,
                ));
                continue;
            }
            // The chunk's own tokens ride along (`'a` and, if written, its
            // `: 'b` bound) — the lifetime node is a leaf.
            binder.push(TyLifetime(chunk.iter().cloned().collect()).to_ty());
        }
    }
    let inner = crate::parse::chain::parse_dot_chain(cursor, ctx.in_bound()).unwrap_or_else(empty);
    TyWithFor(binder, Box::new(inner)).to_ty()
}

/// `dyn ...` — a trait object. Both halves are parsed **structurally**: the
/// qualified type after `dyn` (so `dyn Fn.().3` runs the Fn generator) and the
/// `+ Bound` tail, which becomes a [`TyBoundList`] like every other bound list —
/// so the empty-bracket sync (`X<>`, `sync::sync_bound_ty`) reaches it, and the
/// `+` tokens live in the renderer instead of in the data. A trait object **is** a
/// bound position, so `dyn Iterator<Item = u8>` takes its bindings.
pub(crate) fn dyn_block(cursor: &mut Cursor, ctx: Ctx<'_>) -> Ty {
    cursor.bump(); // `dyn`
    let ctx = ctx.in_bound();
    let inner = crate::parse::chain::parse_dot_chain(cursor, ctx).unwrap_or_else(empty);
    let mut bounds = vec![];
    while cursor.is_punct('+') {
        cursor.bump();
        // A `+` with no bound after it (`dyn Trait +`) is a user error, not a
        // token to emit: the old code pushed the bare `+` and let rustc report
        // it. `starts_block` (or a second `+`, which the next round consumes)
        // decides whether a bound follows.
        if !(starts_block(cursor) || cursor.is_punct('+')) {
            let span = cursor.peek().map_or_else(proc_macro2::Span::call_site, |t| t.span());
            bounds.push(err_ty_at(
                "batch-impl: a `+` in a `dyn` bound list needs a bound after it \
                 (e.g. `dyn Iterator<Item = u8> + Send`)",
                span,
            ));
            continue;
        }
        bounds.push(crate::parse::chain::parse_dot_chain(cursor, ctx).unwrap_or_else(empty));
    }
    TyWithDyn(Box::new(inner), TyBoundList(bounds)).to_ty()
}

/// `impl Trait` — the keyword is fixed syntax, but each bound remains a Ty
/// (a block after the `+` chain is the outer chain's next block).
fn impl_trait_block(cursor: &mut Cursor, ctx: Ctx<'_>) -> Ty {
    cursor.bump(); // `impl`
    let ctx = ctx.in_bound();
    let inner = parse_block(cursor, ctx).unwrap_or_else(empty);
    let mut bounds = vec![];
    while cursor.is_punct('+') {
        cursor.bump();
        bounds.push(parse_block(cursor, ctx).unwrap_or_else(empty));
    }
    let inner = if bounds.is_empty() {
        inner
    } else {
        bounds.insert(0, inner);
        TyBoundList(bounds).to_ty()
    };
    TyPrefixed(quote!(impl), inner.into()).to_ty()
}

/// Plain ident: `::` path segments, a `!` macro call, a **trailing `<>`
/// argument group** (`Box<u8>` — the args belong to the ident, so `X Box<u8>`
/// applies the whole generic), a **qualified tail** (`Foo<T>::Assoc`,
/// `Foo::<u8>::Assoc`), or a bare ident (a trait head when it matches the
/// annotated trait).
pub(crate) fn plain_ident_block(cursor: &mut Cursor, id: Ident, ctx: Ctx<'_>) -> Ty {
    let tokens = vec![TokenTree::Ident(id.clone())];
    cursor.bump();
    plain_ident_path(cursor, tokens, ctx)
}

/// `::path` — a **global path** head (`::std::vec::Vec<u8>`). The `::` is
/// consumed here and handed to the ident parser as the head's prefix, so the
/// one path parser keeps owning segments, turbofish, args and qualified tails.
pub(crate) fn global_path_block(cursor: &mut Cursor, ctx: Ctx<'_>) -> Option<Ty> {
    let (Some(first), Some(second)) = (cursor.peek(), cursor.peek_at(1)) else {
        return None;
    };
    let Some(TokenTree::Ident(id)) = cursor.peek_at(2) else {
        let span = cursor.peek().map_or_else(proc_macro2::Span::call_site, |t| t.span());
        cursor.advance(2);
        return Some(crate::apply::err_ty_at(
            "batch-impl: `::` must be followed by a path segment identifier \
             (e.g. `::std::vec::Vec`)",
            span,
        ));
    };
    let (prefix, id) = (vec![first.clone(), second.clone()], id.clone());
    cursor.advance(2);
    let mut tokens = prefix;
    tokens.push(TokenTree::Ident(id));
    cursor.bump();
    Some(plain_ident_path(cursor, tokens, ctx))
}

/// The tail of [`plain_ident_block`] / [`global_path_block`]: `tokens` already
/// holds the head's spelling (an optional leading `::` plus the first
/// identifier) and the cursor sits after it.
fn plain_ident_path(cursor: &mut Cursor, mut tokens: Vec<TokenTree>, ctx: Ctx<'_>) -> Ty {
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
                // The `::` operator dictionary guarantees both colons; they
                // are extracted rather than unwrapped (no-panic promise).
                let (Some(first), Some(second)) = (cursor.peek(), cursor.peek_at(1)) else {
                    break;
                };
                let seg = seg.clone();
                tokens.push(first.clone());
                tokens.push(second.clone());
                tokens.push(TokenTree::Ident(seg));
                cursor.advance(3);
            }
            // `ident!(...)` macro call — passthrough
            Some(TokenTree::Punct(p)) if p.as_char() == '!' => {
                let Some(TokenTree::Group(g)) = cursor.peek_at(1) else {
                    break;
                };
                tokens.push(TokenTree::Punct(p.clone()));
                tokens.push(TokenTree::Group(g.clone()));
                cursor.advance(2);
                break;
            }
            _ => break,
        }
    }
    // A `<...>` group that is a **qualified-self head** followed by `::` is not
    // this ident's argument list: `Wrapper<T>` takes args, while
    // `M2 <S as Tr>::Assoc` is the trait head `M2` applied to the projection
    // `<S as Tr>::Assoc`. The depth-0 `as` is the discriminator the block parser
    // uses too ([`split_projection`]) — one rule, one authority. Returning the
    // bare head leaves the group for the chain, which parses it as a projection
    // (and the `::` belongs to that projection, so no tail is collected here).
    let projection_ahead = matches!(cursor.peek(), Some(TokenTree::Group(g))
        if g.delimiter() == delimiter![<>]
            && matches!(cursor.op_at(1), Some((crate::util::Op::ColonColon, _)))
            && split_projection(&g.stream().into_iter().collect::<Vec<_>>()).is_some());
    if projection_ahead {
        return bare_head(tokens, ctx);
    }
    // `Foo::<u8>` — a turbofish is the same args list one `::` later; the `::`
    // is dropped and the group folded onto the base below, because in type
    // position `Foo<u8>` and `Foo::<u8>` are the same type (and the turbofish
    // spelling used to be a parse error, so nothing existing changes).
    if matches!(cursor.peek_op(), Some((crate::util::Op::ColonColon, _)))
        && matches!(cursor.peek_at(2), Some(TokenTree::Group(g)) if g.delimiter() == delimiter![<>])
    {
        cursor.advance(2);
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
            Some(TokenTree::Ident(i)) if ctx.trait_name.is_some_and(|tn| tn == &i));
        // Bindings/bounds in the args are valid on a trait path
        // (`Conv<Item = u32> X`) **and in a bound position**
        // (`T: Iterator<Item = u8>` / `dyn Iterator<Item = u8>`) — a plain
        // type's args are a plain type list. The position, not the shape of the
        // list, is what decides (see `parse::generic::ArgsPosition`).
        let position = if is_trait_head || ctx.bound {
            crate::parse::generic::ArgsPosition::TraitPath
        } else {
            crate::parse::generic::ArgsPosition::PlainType
        };
        let params = parse_angle_bracket_contents(&args, ctx, position);
        let head = if is_trait_head {
            // trait head with args (`Tr<A>`) — apply turns it into the impl
            QualifiedHead::Type(Box::new(TyTrait(base_tokens, params).to_ty()))
        } else {
            QualifiedHead::Type(Box::new(
                TyGeneric(TyPrimitive(base_tokens).to_ty().into(), params).to_ty(),
            ))
        };
        return qualified_tail(cursor, head, ctx);
    }
    qualified_tail(cursor, QualifiedHead::Type(Box::new(bare_head(tokens, ctx))), ctx)
}

/// The bare head of a plain ident block: a single ident that names the annotated
/// trait is a trait head (`TyTrait`), anything else is a primitive path. Shared
/// by the ordinary tail of [`plain_ident_block`] and the projection-ahead
/// early return, so the two cannot drift on what a "trait head" is.
fn bare_head(tokens: Vec<TokenTree>, ctx: Ctx<'_>) -> Ty {
    if let [TokenTree::Ident(single)] = tokens.as_slice()
        && ctx.trait_name.is_some_and(|t| t == single)
    {
        return TyTrait(
            TokenStream::from(TokenTree::Ident(single.clone())),
            TyTypeParam { params: vec![], bindings: vec![], fresh_groups: vec![] },
        )
        .to_ty();
    }
    TyPrimitive(tokens.into_iter().collect()).to_ty()
}

/// Collects the `::`-tail after a parsed head — `Foo<T>::Assoc`,
/// `Foo::<u8>::Assoc`, `<T as Tr>::Assoc` — and returns the head **unchanged**
/// when no `::` follows, which is the ordinary path case, so nothing that parses
/// today changes.
///
/// Each segment has a fixed Rust name and optional structural arguments.
/// Argument parsing is shared with ordinary generics; no application is
/// replayed on the completed path or on a segment's identifier.
pub(crate) fn qualified_tail(cursor: &mut Cursor, head: QualifiedHead, ctx: Ctx<'_>) -> Ty {
    let mut tail = vec![];
    while matches!(cursor.peek_op(), Some((crate::util::Op::ColonColon, _))) {
        // The `::` is **consumed** in both error arms, so the chain cannot
        // overwrite the diagnostic here with its own "unexpected `:` after the
        // type" (which points at the wrong token).
        let Some(next) = cursor.peek_at(2) else {
            // `A::` with nothing after it: the segment is *missing*, which is a
            // different mistake from a DSL token in the tail — and the only token
            // to point at is the `::` itself.
            let span = cursor.peek().map_or_else(proc_macro2::Span::call_site, |t| t.span());
            cursor.advance(2);
            return crate::apply::err_ty_at(
                "batch-impl: `::` must be followed by a path segment (write `Foo::Assoc`)",
                span,
            );
        };
        let TokenTree::Ident(seg) = next else {
            // `::` followed by anything but an identifier is not a path tail
            // (`<T as Tr>::@0` / `::#cmd`).
            let span = cursor.peek().map_or_else(proc_macro2::Span::call_site, |t| t.span());
            cursor.advance(2);
            return crate::apply::err_ty_at(
                "batch-impl: a `::`-tail segment must be an identifier — DSL tokens \
                 (`@…` / `#…`) are not allowed in a `::`-tail",
                span,
            );
        };
        let seg = seg.clone();
        cursor.advance(3);
        // `::Assoc<u8>` — a tail segment's own args (GAT-style tail).
        let args = if let Some(g) = cursor.peek_group(delimiter![<>]) {
            cursor.bump();
            Some(parse_angle_bracket_contents(
                &g.stream().into_iter().collect::<Vec<_>>(),
                ctx.plain(),
                crate::parse::generic::ArgsPosition::PlainType,
            ))
        } else {
            None
        };
        tail.push(TyPathSegment { ident: seg, args });
    }
    match (head, tail.is_empty()) {
        (QualifiedHead::Type(t), true) => *t,
        // A `Projection` head with no tail (`<T as Tr>` alone) is not a type on
        // its own: keep the node so the render is faithful and rustc reports it.
        (head, _) => TyQualified(head, tail).to_ty(),
    }
}

/// Splits the contents of a leading `<...>` angle group into a **qualified-self**
/// head: `<T as Tr>` → the projection type (`T`) and the trait path (`Tr`).
/// Returns `None` unless a **depth-0** `as` is present — the only content shape
/// that can be a projection (a plain args list like `<u8, u16>` cannot).
pub(crate) fn split_projection(args: &[TokenTree]) -> Option<(Vec<TokenTree>, TokenStream)> {
    let at = args.iter().position(|t| matches!(t, TokenTree::Ident(id) if id == "as"))?;
    let ty = crate::util::slice_upto(args, at).to_vec();
    if ty.is_empty() {
        return None;
    }
    let trait_tokens = crate::preprocess::render_angles(
        crate::util::slice_from(args, at + 1).iter().cloned().collect(),
    );
    if trait_tokens.is_empty() {
        return None;
    }
    Some((ty, trait_tokens))
}
