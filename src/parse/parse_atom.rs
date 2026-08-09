use crate::apply::{err_ty, err_ty_at};
use crate::ast::*;
use crate::parse::generic::empty;
use crate::parse::{parse_item, parse_primitive};
use crate::util::{Cursor, contains_punct};
use proc_macro2::{Delimiter, Ident, Spacing, TokenStream, TokenTree};

/// `#[...]` attribute parsing
pub(crate) fn parse_attribute(
    tokens: &[TokenTree],
) -> Option<(TokenStream, &[TokenTree])> {
    match tokens {
        [TokenTree::Punct(hash), TokenTree::Group(group), rest @ ..]
            if hash.as_char() == '#' && group.delimiter() == delimiter![[]] =>
        {
            (group.stream(), rest).into()
        }
        _ => None,
    }
}

/// `fn(A,B)->C` function type parsing (fn + parameter tuple + optional return type)
pub(crate) fn parse_function(
    tokens: &[TokenTree], trait_name: Option<&Ident>,
) -> Option<Ty> {
    let [TokenTree::Ident(name), TokenTree::Group(args), rest @ ..] = tokens else {
        return None;
    };
    if name != "fn" || args.delimiter() != delimiter![()] {
        return None;
    }
    let fn_span = name.span();

    let args_tokens = args.stream().into_iter().collect::<Vec<_>>();
    let mut cursor = Cursor::new(&args_tokens);
    let mut parameters = vec![];
    if cursor.is_punct(',') {
        return err_ty_at(
            "batch-impl: `fn` parameter list cannot start with `,`",
            args.span(),
        )
        .into();
    }
    while let Some(parameter) = parse_item(&mut cursor, Op::Comma, trait_name) {
        parameters.push(parameter);
    }

    let return_type = match rest {
        [TokenTree::Punct(dash), TokenTree::Punct(arrow), return_tokens @ ..]
            if dash.as_char() == '-'
                && dash.spacing() == Spacing::Joint
                && arrow.as_char() == '>'
                && !return_tokens.is_empty() =>
        {
            parse_primitive(return_tokens, trait_name).into()
        }
        _ => None,
    };
    TyFn(parameters.into(), return_type, false).to_ty().with_span(fn_span).into()
}

/// Prefix modifier parsing: `&`/`&mut`/`*const`/`*mut`/`self`/`unsafe`
/// (`fn` is handled by `parse_function` or the bare-`fn` branch in parse.rs)
pub(crate) fn parse_prefix(tokens: &[TokenTree]) -> Option<(TyPrefix, &[TokenTree])> {
    match tokens {
        [TokenTree::Punct(p), TokenTree::Ident(name), rest @ ..]
            if p.as_char() == '&' && name == "mut" =>
        {
            (TyPrefix::RefMut, rest).into()
        }
        [TokenTree::Punct(p), rest @ ..] if p.as_char() == '&' => {
            (TyPrefix::Ref, rest).into()
        }
        [TokenTree::Punct(p), TokenTree::Ident(name), rest @ ..]
            if p.as_char() == '*' && name == "const" =>
        {
            (TyPrefix::PtrConst, rest).into()
        }
        [TokenTree::Punct(p), TokenTree::Ident(name), rest @ ..]
            if p.as_char() == '*' && name == "mut" =>
        {
            (TyPrefix::PtrMut, rest).into()
        }
        [TokenTree::Ident(name), rest @ ..] if name == "self" => {
            (TyPrefix::SelfType, rest).into()
        }
        [TokenTree::Ident(name), rest @ ..] if name == "unsafe" => {
            (TyPrefix::Unsafe, rest).into()
        }
        _ => None,
    }
}

/// `N..M` / `N..=M` range parsing
pub(crate) fn parse_range(tokens: &[TokenTree]) -> Option<Ty> {
    let [
        TokenTree::Literal(start),
        TokenTree::Punct(first_dot),
        TokenTree::Punct(second_dot),
        rest @ ..,
    ] = tokens
    else {
        return None;
    };
    if first_dot.as_char() != '.'
        || second_dot.as_char() != '.'
        || first_dot.spacing() != Spacing::Joint
    {
        return None;
    }
    let start = start.to_string().parse::<usize>().ok()?;
    let span = tokens[0].span();
    let (inclusive, end) = match rest {
        [TokenTree::Literal(end)] => (false, end),
        [TokenTree::Punct(eq), TokenTree::Literal(end)]
            if eq.as_char() == '=' && second_dot.spacing() == Spacing::Joint =>
        {
            (true, end)
        }
        _ => return None,
    };
    TyRange { start, end: end.to_string().parse().ok()?, inclusive }
        .to_ty()
        .with_span(span)
        .into()
}

/// Group parsing: `(A,B)` tuple / `(A)` group / `[A,B]` list / `[A; N]` array / `[A]` slice /
/// Whether a group's content is a **lone splat**: exactly a `*` punct
/// followed by a `(...)` / `[...]` group (`(*(a,b))`, `[*(a,b)]`,
/// `(*[a,b])`, `[*[a,b]]`). Such a group is a list whose content is the
/// splat — the group auto-becomes the matching container (tuple for `()`,
/// array for `[]`), unifying `(*(a,b))` ≡ `(*(a,b),)` and
/// `[*(a,b)]` ≡ `[*(a,b),]` on one code path.
fn lone_splat(contents: &[TokenTree]) -> bool {
    matches!(
        contents,
        [TokenTree::Punct(p), TokenTree::Group(g)]
            if p.as_char() == '*'
                && matches!(
                    g.delimiter(),
                    Delimiter::Parenthesis | Delimiter::Bracket
                )
    )
}

/// `{...}` code block
pub(crate) fn parse_group(
    group: &proc_macro2::Group, trait_name: Option<&Ident>,
) -> Ty {
    let contents = group.stream().into_iter().collect::<Vec<_>>();
    match group.delimiter() {
        delimiter![()] => {
            // Unified rule: a group whose content is empty, comma-separated,
            // or a **lone splat** (`(*(a,b))` / `(*[a,b])`) is a list —
            // the splat is the list's content and the group auto-becomes the
            // matching container (tuple). `(*(a,b))` ≡ `(*(a,b),)` ≡ `(a,b)`
            // — one code path, no special case. Non-splat single-element
            // groups (`(a)`) stay transparent groups (`TyGroup`).
            if contents.is_empty()
                || contains_punct(&contents, ',')
                || lone_splat(&contents)
            {
                // Splat elements are KEPT (splat survival: parse never
                // flattens `*()`/`*[]` — `(a, *(b,c))` stays a tuple with a
                // splat element; codegen expands it into `(a, b, c)`).
                TyTuple(parse_list(&contents, Op::Comma, trait_name))
                    .to_ty()
                    .with_span(group.span())
            } else {
                let inner =
                    parse_item(&mut Cursor::new(&contents), Op::Dash, trait_name)
                        .unwrap_or_else(empty);
                TyGroup(Box::new(inner)).to_ty().with_span(group.span())
            }
        }
        delimiter![[]] => {
            // With a comma it is a list; a **lone splat** (`[*(a,b)]` /
            // `[*[a,b]]`) is also a list — the splat auto-becomes the
            // array's content (`[*(a,b)]` ≡ `[*(a,b),]` ≡ `[a,b]` as
            // impl-list / dispatch). `;` (Op::Semi) distinguishes arrays
            // from slices; empty `[]` is the array/slice builder base.
            if contains_punct(&contents, ',') || lone_splat(&contents) {
                // Splat elements are KEPT (not consumed here) — a splat lives
                // until consumption (apply-right / codegen), per the splat
                // survival principle: `[*(A),*(B)]^2` repeats each element
                // (`[*(A,A),*(B,B)]`) instead of flattening to bare types.
                let flat = parse_list(&contents, Op::Comma, trait_name);
                TyArray(flat).to_ty().with_span(group.span())
            } else if contents.is_empty() {
                TyPrimitiveArray(None, None).to_ty().with_span(group.span())
            } else {
                let mut cursor = Cursor::new(&contents);
                let element = parse_item(&mut cursor, Op::Semi, trait_name)
                    .unwrap_or_else(empty);
                if cursor.is_punct(';') {
                    cursor.bump();
                    let length =
                        cursor.take_rest().iter().cloned().collect::<TokenStream>();
                    TyPrimitiveArray(element.into(), length.into())
                        .to_ty()
                        .with_span(group.span())
                } else {
                    TyPrimitiveArray(element.into(), None)
                        .to_ty()
                        .with_span(group.span())
                }
            }
        }
        delimiter![{}] => TyWithCode(None, TyCodeBlock(group.stream()))
            .to_ty()
            .with_span(group.span()),
        _ => empty(),
    }
}

/// Parse a list by looping at the given level (stops when `parse_item` returns None)
pub(crate) fn parse_list(
    tokens: &[TokenTree], level: Op, trait_name: Option<&Ident>,
) -> Vec<Ty> {
    let mut cursor = Cursor::new(tokens);
    let mut items = vec![];
    // Leading comma (`[,A]` / `(,A)`): a list starting with `,` is a typo
    if cursor.is_punct(',') {
        items.push(err_ty("batch-impl: a list cannot start with `,`"));
    }
    while let Some(item) = parse_item(&mut cursor, level, trait_name) {
        items.push(item);
    }
    items
}
