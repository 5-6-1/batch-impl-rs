use crate::apply::{err_ty, err_ty_at};
use crate::ast::*;
use crate::parse::generic::empty;
use crate::parse::{parse_item, parse_primitive};
use crate::util::{Cursor, contains_punct};
use proc_macro2::{Ident, Spacing, TokenStream, TokenTree};

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
/// `{...}` code block
pub(crate) fn parse_group(
    group: &proc_macro2::Group, trait_name: Option<&Ident>,
) -> Ty {
    let contents = group.stream().into_iter().collect::<Vec<_>>();
    match group.delimiter() {
        delimiter![()] => {
            if contents.is_empty() || contains_punct(&contents, ',') {
                let (flat, decl) =
                    consume_splats(parse_list(&contents, Op::Comma, trait_name));
                let tuple = TyTuple(flat).to_ty().with_span(group.span());
                match decl {
                    Some(d) => {
                        TyWithType(d, tuple.into()).to_ty().with_span(group.span())
                    }
                    None => tuple,
                }
            } else {
                let inner =
                    parse_item(&mut Cursor::new(&contents), Op::Dash, trait_name)
                        .unwrap_or_else(empty);
                // Group → tuple: a single-element group whose content is a
                // splat flattens into multiple elements (`(*(a,b))` → `(a,b)`).
                if matches!(inner.kind, TyKind::Splat(_)) {
                    let (flat, decl) = consume_splats(vec![inner]);
                    let tuple = TyTuple(flat).to_ty().with_span(group.span());
                    match decl {
                        Some(d) => TyWithType(d, tuple.into())
                            .to_ty()
                            .with_span(group.span()),
                        None => tuple,
                    }
                } else {
                    TyGroup(Box::new(inner)).to_ty().with_span(group.span())
                }
            }
        }
        delimiter![[]] => {
            // With a comma it is a list; otherwise `;` (Op::Semi) distinguishes arrays from slices.
            // Empty `[]` is the array/slice builder base `(None, None)`.
            if contains_punct(&contents, ',') {
                let (flat, decl) =
                    consume_splats(parse_list(&contents, Op::Comma, trait_name));
                let arr = TyArray(flat).to_ty().with_span(group.span());
                match decl {
                    Some(d) => {
                        TyWithType(d, arr.into()).to_ty().with_span(group.span())
                    }
                    None => arr,
                }
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
                } else if matches!(element.kind, TyKind::Splat(_)) {
                    // `[*(a,b)]` — a lone splat at the slice position (no
                    // comma) flattens into a list, matching `(*(a,b))` →
                    // `(a,b)` (syntax parity between `[]` and `()`).
                    let (flat, decl) = consume_splats(vec![element]);
                    let arr = TyArray(flat).to_ty().with_span(group.span());
                    match decl {
                        Some(d) => {
                            TyWithType(d, arr.into()).to_ty().with_span(group.span())
                        }
                        None => arr,
                    }
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

/// Consume splat elements in a collected list: only `TySplat` elements are
/// flattened (containers and generators inside them — `[a, *[d,e,f]]` →
/// `[a,d,e,f]`); ordinary elements stay untouched (`(a, ()^3)` keeps its
/// nested generator). Returns the flat list plus the merged fresh declaration
/// for the enclosing container (wrapped in `WithType` by the caller).
fn consume_splats(items: Vec<Ty>) -> (Vec<Ty>, Option<TyTypeParam>) {
    let mut flat = vec![];
    let mut decl = None;
    for item in items {
        if matches!(item.kind, TyKind::Splat(_)) {
            let (mut es, d) = splat_expand(item);
            flat.append(&mut es);
            decl = merge_decls(decl, d);
        } else {
            flat.push(item);
        }
    }
    (flat, decl)
}
