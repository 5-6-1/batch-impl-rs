//! Space-application block parsing: `Box u8` / `HashMap u32 String` —
//! adjacent blocks separated by a space are a left-associative application
//! (the successor of the `-` operator). The space is not a token, so the
//! chain cuts at **block boundaries**: a block is the smallest
//! self-contained type fragment (`&` / `&mut` / `*const` / `fn(...)` /
//! `<...>` / `{...}` / an ident, a group, a number, ...), and the chain
//! folds blocks with `apply`. The `.` operator is a chain-level operator
//! too (right-assoc, higher precedence than the space) — see `chain.rs`.
//!
//! Precedence (low → high): space (left-assoc) < `.` (right-assoc) < block.

use crate::apply::err_ty_at;
use crate::ast::fresh::at_ref_name;
use crate::ast::*;
use crate::parse::chain::parse_dot_chain;
use crate::parse::generic::{empty, parse_angle_bracket_contents, split_at_depth0};
use crate::parse::parse_atom::{parse_group, parse_range};
use crate::parse::parse_item;
use crate::util::Cursor;
use proc_macro2::{Delimiter, Ident, Spacing, TokenStream, TokenTree};
use quote::{ToTokens, quote};

/// Whether the token opens a new block: any ident/literal/group or a
/// block-opening punct (`&` `*` `?` `!` `@` `'` `#`). Operators and
/// separators (`.` `,` `;` `-` `:` `+` `>` `=`) do not open blocks.
pub(crate) fn starts_block(t: &TokenTree) -> bool {
    matches!(t, TokenTree::Ident(_) | TokenTree::Literal(_))
        || matches!(t, TokenTree::Group(_))
        || matches!(t, TokenTree::Punct(p)
            if matches!(p.as_char(), '&' | '*' | '?' | '!' | '@' | '\'' | '#'))
}

/// Whether the cursor sits on the first `.` of a `..` range (a Joint `.`
/// whose next token is another `.`).
pub(crate) fn cursor_is_dotdot(cursor: &Cursor) -> bool {
    matches!(cursor.peek(), Some(TokenTree::Punct(p))
        if p.as_char() == '.' && p.spacing() == Spacing::Joint
            && matches!(cursor.peek_at(1), Some(TokenTree::Punct(q)) if q.as_char() == '.'))
}

/// Whether the cursor sits on a `->` fn arrow (Joint `-` followed by `>`).
fn cursor_is_arrow(cursor: &Cursor) -> bool {
    matches!(cursor.peek(), Some(TokenTree::Punct(p))
        if p.as_char() == '-' && p.spacing() == Spacing::Joint
            && matches!(cursor.peek_at(1), Some(TokenTree::Punct(q)) if q.as_char() == '>'))
}

/// `'a` lifetime tokens (`'` punct + ident).
fn lifetime_tokens(lt: &Ident) -> TokenStream {
    let mut ts = TokenStream::from(TokenTree::Punct(proc_macro2::Punct::new('\'', Spacing::Joint)));
    ts.extend(TokenStream::from(TokenTree::Ident(lt.clone())));
    ts
}

/// Whether the next tokens open an **attachment** block (`{...}` /
/// `where{...}` / `impl{...}`) — the return-expression chain stops there.
fn cursor_at_attachment(cursor: &Cursor) -> bool {
    match cursor.peek() {
        Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Brace => true,
        Some(TokenTree::Ident(id))
            if (id == "where" || id == "impl")
                && matches!(cursor.peek_at(1), Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Brace) =>
        {
            true
        }
        _ => false,
    }
}

/// Parses one **block** from the cursor: the smallest self-contained type
/// fragment plus its fixed suffixes. Returns `None` when the cursor is not at
/// a block start (an operator / separator / the end).
///
/// A block never swallows the type it would apply to (`&mut u8` is the two
/// blocks `&mut` and `u8`, folded by the chain) — with two exceptions where
/// Rust syntax forces the fragment together: lifetime references
/// (`&'a mut u8`) and the fn family (`fn(u8) -> u8`).
pub(crate) fn parse_block(cursor: &mut Cursor, trait_name: Option<&Ident>) -> Option<Ty> {
    let ty: Ty = match cursor.peek()? {
        // `#[attr]` — attribute block (the chain applies the next block)
        TokenTree::Punct(p)
            if p.as_char() == '#'
                && matches!(cursor.peek_at(1), Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Bracket) =>
        {
            let attr = match cursor.peek_at(1) {
                Some(TokenTree::Group(g)) => g.stream(),
                _ => unreachable!(),
            };
            cursor.advance(2);
            TyWithAttr(TyAttr(attr), None).to_ty()
        }
        // `{body}` code block (incl. `#name{...}` directive products) and
        // the `{! ...}` top-level macro form — chain blocks now.
        TokenTree::Group(g) if g.delimiter() == Delimiter::Brace => {
            let body = g.stream();
            cursor.bump();
            TyWithCode(None, TyCodeBlock(body)).to_ty()
        }
        // `(...)` tuple / `[...]` list-array — the whole group is one block
        TokenTree::Group(g) if g.delimiter() != Delimiter::None => {
            let g = g.clone();
            cursor.bump();
            parse_group(&g, trait_name)
        }
        // `<...>` alone — a generic declaration/args list (TyTypeParam);
        // whether it is a declaration or args is decided by apply.
        TokenTree::Group(g) => {
            let args = g.stream().into_iter().collect::<Vec<_>>();
            cursor.bump();
            parse_angle_bracket_contents(&args, trait_name, true).to_ty()
        }
        // `&` / `&mut` / `&'a` / `&'a mut`
        TokenTree::Punct(p) if p.as_char() == '&' => reference_block(cursor, trait_name),
        // `*const` / `*mut` / `*[...]` / `*(...)`
        TokenTree::Punct(p) if p.as_char() == '*' => star_block(cursor),
        // `@N` position reference
        TokenTree::Punct(p) if p.as_char() == '@' => at_ref_block(cursor),
        // `'a` lifetime
        TokenTree::Punct(p)
            if p.as_char() == '\'' && matches!(cursor.peek_at(1), Some(TokenTree::Ident(_))) =>
        {
            let lt = match cursor.peek_at(1) {
                Some(TokenTree::Ident(id)) => Ident::new(&id.to_string(), id.span()),
                _ => unreachable!(),
            };
            cursor.advance(2);
            TyPrimitive(lifetime_tokens(&lt)).to_ty()
        }
        // `?` / `!` prefix puncts — swallow the qualified type (passthrough)
        TokenTree::Punct(p) if matches!(p.as_char(), '?' | '!') => {
            let p = p.as_char();
            cursor.bump();
            let inner = parse_block(cursor, trait_name).unwrap_or_else(empty);
            let p_tt = TokenTree::Punct(proc_macro2::Punct::new(p, Spacing::Alone));
            TyPrimitive(quote!(#p_tt #inner)).to_ty()
        }
        // numbers / ranges
        TokenTree::Literal(_) => literal_block(cursor),
        TokenTree::Ident(id) => ident_block(cursor, id.clone(), trait_name),
        _ => return None,
    };
    Some(ty)
}

/// `&` block family: `&` / `&mut` / `&'a` / `&'a mut` — the prefix never
/// swallows the target type (`&mut u8` = `&mut` + `u8`), except a lifetime
/// reference which is one block (`&'a mut u8` — a bare `&'a` is not a type).
fn reference_block(cursor: &mut Cursor, trait_name: Option<&Ident>) -> Ty {
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
fn star_block(cursor: &mut Cursor) -> Ty {
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
                .map(|c| parse_item(&mut Cursor::new(c), Op::Dash, None).unwrap_or_else(empty))
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
fn at_ref_block(cursor: &mut Cursor) -> Ty {
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
fn literal_block(cursor: &mut Cursor) -> Ty {
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

/// Ident block: `::` paths (`std::vec::Vec`), macro calls (`m!(...)`), the
/// fn family (`fn` / `unsafe fn` / `extern "C" fn`), the trait-object
/// families (`dyn ...` / `for ...` / `Fn ...` / `impl Trait`), the
/// `impl{...}` / `where{...}` attachment blocks, or a bare ident (a trait
/// head when it matches the annotated trait).
fn ident_block(cursor: &mut Cursor, id: Ident, trait_name: Option<&Ident>) -> Ty {
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
        "dyn" => swallow_chain(cursor, &id, trait_name),
        "for" => for_block(cursor, trait_name),
        "Fn" | "FnMut" | "FnOnce" if matches!(cursor.peek_at(1), Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Parenthesis) => {
            fn_trait_block(cursor, &id)
        }
        "impl" if matches!(cursor.peek_at(1), Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Brace) =>
        {
            let g = match cursor.peek_at(1) {
                Some(TokenTree::Group(g)) => g.clone(),
                _ => unreachable!(),
            };
            cursor.advance(2);
            TyWithImpl(None, TyImplTemplate(g.stream())).to_ty()
        }
        "impl" => swallow_chain(cursor, &id, trait_name),
        "where" if matches!(cursor.peek_at(1), Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Brace) =>
        {
            let g = match cursor.peek_at(1) {
                Some(TokenTree::Group(g)) => g.clone(),
                _ => unreachable!(),
            };
            cursor.advance(2);
            TyWithWhere(None, TyWhere(g.stream())).to_ty()
        }
        _ => plain_ident_block(cursor, id, trait_name),
    }
}

/// fn family: `fn` / `unsafe fn` — the parameter group is consumed, and an
/// optional `-> Ret` return type (a full space expression that stops at an
/// attachment block — `{...}` / `where{...}` / `impl{...}` belong to the
/// impl, not to the fn type). A bare `fn` keeps its params to be filled by
/// `.` later.
fn fn_block(cursor: &mut Cursor, trait_name: Option<&Ident>, is_unsafe: bool) -> Ty {
    cursor.bump(); // `fn`
    let params = if matches!(cursor.peek(), Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Parenthesis)
    {
        let g = match cursor.peek() {
            Some(TokenTree::Group(g)) => g.clone(),
            _ => unreachable!(),
        };
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
    TyFn(params, ret.map(Into::into), is_unsafe).to_ty()
}

/// `extern "C" fn(...)` — one passthrough block (the ABI literal is not a
/// TyFn field).
fn extern_fn_block(cursor: &mut Cursor) -> Ty {
    let start = cursor.pos();
    cursor.bump(); // `extern`
    cursor.bump(); // `"C"`
    cursor.bump(); // `fn`
    if matches!(cursor.peek(), Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Parenthesis)
    {
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

/// `-> Ret` return expression: blocks folded by the space chain, stopping at
/// an attachment block.
fn parse_return_expr(cursor: &mut Cursor, trait_name: Option<&Ident>) -> Ty {
    let mut left = parse_dot_chain(cursor, trait_name).unwrap_or_else(empty);
    while let Some(t) = cursor.peek() {
        if !starts_block(t) || cursor_at_attachment(cursor) {
            break;
        }
        let right = parse_dot_chain(cursor, trait_name).unwrap_or_else(empty);
        left = left.apply(right);
    }
    left
}

/// A trait bound expression (`Clone + IntoIterator + 'a`): blocks folded by
/// the space chain, then any `+` chain is collected into a passthrough —
/// `+` is a bound operator, not a space application.
pub(crate) fn parse_bound_expr(cursor: &mut Cursor, trait_name: Option<&Ident>) -> Ty {
    let mut left = parse_dot_chain(cursor, trait_name).unwrap_or_else(empty);
    loop {
        match cursor.peek() {
            Some(t) if starts_block(t) && !cursor_at_attachment(cursor) => {
                let right = parse_dot_chain(cursor, trait_name).unwrap_or_else(empty);
                left = left.apply(right);
            }
            _ => break,
        }
    }
    if cursor.is_punct('+') {
        let mut ts = left.to_token_stream();
        while cursor.is_punct('+') {
            ts.extend(cursor.peek().unwrap().to_token_stream());
            cursor.bump();
            if let Some(t) = cursor.peek()
                && starts_block(t)
                && !cursor_at_attachment(cursor)
            {
                let right = parse_dot_chain(cursor, trait_name).unwrap_or_else(empty);
                ts.extend(right.to_token_stream());
            }
        }
        return TyPrimitive(ts).to_ty();
    }
    left
}

/// Consumes the return expression's blocks (used when only the token extent
/// matters — `extern "C" fn` / `Fn(...)` passthrough).
fn parse_return_expr_tokens(cursor: &mut Cursor) {
    if parse_block(cursor, None).is_none() {
        return;
    }
    loop {
        match cursor.peek() {
            Some(t) if starts_block(t) && !cursor_at_attachment(cursor) => {
                let _ = parse_block(cursor, None);
            }
            _ => break,
        }
    }
}

/// `Fn(A) -> B` — fn-trait call block, rendered as a passthrough.
fn fn_trait_block(cursor: &mut Cursor, _id: &Ident) -> Ty {
    let start = cursor.pos();
    cursor.bump(); // `Fn`
    if matches!(cursor.peek(), Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Parenthesis)
    {
        cursor.bump();
    }
    if cursor_is_arrow(cursor) {
        cursor.advance(2);
        parse_return_expr_tokens(cursor);
    }
    let n = cursor.pos() - start;
    let tokens = cursor.slice_at(start, n).to_vec();
    TyPrimitive(tokens.into_iter().collect()).to_ty()
}

/// `for<'a> fn(...)` — swallow the HRTB bound group + qualified type.
fn for_block(cursor: &mut Cursor, trait_name: Option<&Ident>) -> Ty {
    let start = cursor.pos();
    cursor.bump(); // `for`
    if matches!(cursor.peek(), Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::None) {
        cursor.bump();
        parse_block(cursor, trait_name).unwrap_or_else(empty);
    }
    let n = cursor.pos() - start;
    let tokens = cursor.slice_at(start, n).to_vec();
    TyPrimitive(tokens.into_iter().collect()).to_ty()
}

/// `dyn ...` / `impl Trait` — swallow the qualified type and a `+ Bound`
/// chain (a block after the chain ends is the chain's next block).
fn swallow_chain(cursor: &mut Cursor, _id: &Ident, trait_name: Option<&Ident>) -> Ty {
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
fn plain_ident_block(cursor: &mut Cursor, id: Ident, trait_name: Option<&Ident>) -> Ty {
    let mut tokens = vec![TokenTree::Ident(id.clone())];
    cursor.bump();
    loop {
        match cursor.peek() {
            // `::` path segment stays in the block
            Some(TokenTree::Punct(p))
                if p.as_char() == ':'
                    && p.spacing() == Spacing::Joint
                    && matches!(cursor.peek_at(1), Some(TokenTree::Punct(q)) if q.as_char() == ':')
                    && matches!(cursor.peek_at(2), Some(TokenTree::Ident(_))) =>
            {
                let seg = match cursor.peek_at(2) {
                    Some(TokenTree::Ident(s)) => s.clone(),
                    _ => unreachable!(),
                };
                tokens.push(TokenTree::Punct(p.clone()));
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
        && g.delimiter() == Delimiter::None
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

// ------------------------------------------------------------
// Cursor helpers
// ------------------------------------------------------------

fn peek_ident_at(cursor: &Cursor, off: usize, name: &str) -> bool {
    matches!(cursor.peek_at(off), Some(TokenTree::Ident(id)) if id == name)
}

fn cursor_is_lifetime(cursor: &Cursor) -> bool {
    matches!(cursor.peek(), Some(TokenTree::Punct(p)) if p.as_char() == '\'')
        && matches!(cursor.peek_at(1), Some(TokenTree::Ident(_)))
}
