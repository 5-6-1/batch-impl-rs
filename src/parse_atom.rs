use proc_macro2::{Delimiter, Ident, Spacing, TokenStream, TokenTree};
use crate::generic::empty;
use crate::parse::{parse_item, parse_primitive};
use crate::scan::{Cursor, contains_punct};
use crate::types::*;

/// `#[...]` 属性解析
pub(crate) fn parse_attribute(
    tokens: &[TokenTree],
) -> Option<(TokenStream, &[TokenTree])> {
    match tokens {
        [TokenTree::Punct(hash), TokenTree::Group(group), rest @ ..]
            if hash.as_char() == '#'
                && group.delimiter() == Delimiter::Bracket =>
        {
            Some((group.stream(), rest))
        },
        _ => None,
    }
}

/// `fn(A,B)->C` 函数类型解析（fn + 参数元组 + 可选返回类型）
pub(crate) fn parse_function(
    tokens: &[TokenTree],
    trait_name: Option<&Ident>,
) -> Option<Ty> {
    let [TokenTree::Ident(name), TokenTree::Group(args), rest @ ..] =
        tokens
    else {
        return None;
    };
    if name != "fn" || args.delimiter() != Delimiter::Parenthesis {
        return None;
    }

    let args_tokens: Vec<_> = args.stream().into_iter().collect();
    let mut cursor = Cursor::new(&args_tokens);
    let mut parameters = Vec::new();
    while let Some(parameter) =
        parse_item(&mut cursor, Op::Comma, trait_name)
    {
        parameters.push(parameter);
    }

    let return_type = match rest {
        [
            TokenTree::Punct(dash),
            TokenTree::Punct(arrow),
            return_tokens @ ..,
        ] if dash.as_char() == '-'
            && dash.spacing() == Spacing::Joint
            && arrow.as_char() == '>'
            && !return_tokens.is_empty() =>
        {
            Some(Box::new(parse_primitive(return_tokens, trait_name)))
        },
        _ => None,
    };
    Some(Ty::Fn(TyFn(parameters, return_type)))
}

/// 前缀修饰符解析：`&`/`&mut`/`*const`/`*mut`/`self`/`fn`/`unsafe`
pub(crate) fn parse_prefix(tokens: &[TokenTree]) -> Option<(TyPrefix, &[TokenTree])> {
    match tokens {
        [TokenTree::Punct(p), TokenTree::Ident(name), rest @ ..]
            if p.as_char() == '&' && name == "mut" =>
        {
            Some((TyPrefix::RefMut, rest))
        },
        [TokenTree::Punct(p), rest @ ..] if p.as_char() == '&' => {
            Some((TyPrefix::Ref, rest))
        },
        [TokenTree::Punct(p), TokenTree::Ident(name), rest @ ..]
            if p.as_char() == '*' && name == "const" =>
        {
            Some((TyPrefix::PtrConst, rest))
        },
        [TokenTree::Punct(p), TokenTree::Ident(name), rest @ ..]
            if p.as_char() == '*' && name == "mut" =>
        {
            Some((TyPrefix::PtrMut, rest))
        },
        [TokenTree::Ident(name), rest @ ..] if name == "self" => {
            Some((TyPrefix::SelfType, rest))
        },
        [TokenTree::Ident(name), rest @ ..] if name == "fn" => {
            Some((TyPrefix::Fn, rest))
        },
        [TokenTree::Ident(name), rest @ ..] if name == "unsafe" => {
            Some((TyPrefix::Unsafe, rest))
        },
        _ => None,
    }
}

/// `N..M` / `N..=M` 范围解析
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
    let start = start.to_string().parse::<u8>().ok()?;
    let (inclusive, end) = match rest {
        [TokenTree::Literal(end)] => (false, end),
        [TokenTree::Punct(eq), TokenTree::Literal(end)]
            if eq.as_char() == '='
                && second_dot.spacing() == Spacing::Joint =>
        {
            (true, end)
        },
        _ => return None,
    };
    Some(Ty::Range(TyRange {
        start,
        end: end.to_string().parse().ok()?,
        inclusive,
    }))
}

/// 分组解析：`(A,B)` 元组 / `(A)` 分组 / `[A,B]` 列表 / `[A; N]` 定长数组 / `[A]` 切片 / `{...}` 代码块
pub(crate) fn parse_group(
    group: &proc_macro2::Group,
    trait_name: Option<&Ident>,
) -> Ty {
    let contents: Vec<_> = group.stream().into_iter().collect();
    match group.delimiter() {
        Delimiter::Parenthesis => {
            if contents.is_empty() || contains_punct(&contents, ',') {
                Ty::Tuple(TyTuple(parse_list(
                    &contents,
                    Op::Comma,
                    trait_name,
                )))
            } else {
                Ty::Group(TyGroup(Box::new(
                    parse_item(
                        &mut Cursor::new(&contents),
                        Op::Dash,
                        trait_name,
                    )
                    .unwrap_or_else(empty),
                )))
            }
        },
        Delimiter::Bracket => {
            // 有逗号是并列列表；否则以 `;`（Op::Semi）区分定长数组与切片
            if contains_punct(&contents, ',') {
                Ty::Array(TyArray(parse_list(
                    &contents,
                    Op::Comma,
                    trait_name,
                )))
            } else {
                let mut cursor = Cursor::new(&contents);
                let element =
                    parse_item(&mut cursor, Op::Semi, trait_name)
                        .unwrap_or_else(empty);
                if cursor.is_punct(';') {
                    cursor.bump();
                    let length =
                        cursor.take_rest().iter().cloned().collect();
                    Ty::FixedArray(TyFixedArray(Box::new(element), length))
                } else {
                    Ty::Slice(TySlice(Box::new(element)))
                }
            }
        },
        Delimiter::Brace => Ty::CodeBlock(TyCodeBlock(group.stream())),
        _ => empty(),
    }
}

/// 按给定优先级循环解析列表（`parse_item` 返回 None 时停止）
pub(crate) fn parse_list(
    tokens: &[TokenTree],
    level: Op,
    trait_name: Option<&Ident>,
) -> Vec<Ty> {
    let mut cursor = Cursor::new(tokens);
    let mut items = Vec::new();
    while let Some(item) = parse_item(&mut cursor, level, trait_name) {
        items.push(item);
    }
    items
}
