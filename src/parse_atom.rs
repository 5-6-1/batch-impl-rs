use crate::generic::empty;
use crate::parse::{parse_item, parse_primitive};
use crate::scan::{Cursor, contains_punct};
use crate::types::*;
use proc_macro2::{Delimiter, Ident, Spacing, TokenStream, TokenTree};

/// `#[...]` 属性解析
pub(crate) fn parse_attribute(
    tokens: &[TokenTree],
) -> Option<(TokenStream, &[TokenTree])> {
    match tokens {
        [TokenTree::Punct(hash), TokenTree::Group(group), rest @ ..]
            if hash.as_char() == '#' && group.delimiter() == Delimiter::Bracket =>
        {
            Some((group.stream(), rest))
        }
        _ => None,
    }
}

/// `fn(A,B)->C` 函数类型解析（fn + 参数元组 + 可选返回类型）
pub(crate) fn parse_function(
    tokens: &[TokenTree], trait_name: Option<&Ident>,
) -> Option<Ty> {
    let [TokenTree::Ident(name), TokenTree::Group(args), rest @ ..] = tokens else {
        return None;
    };
    if name != "fn" || args.delimiter() != Delimiter::Parenthesis {
        return None;
    }

    let args_tokens = args.stream().into_iter().collect::<Vec<_>>();
    let mut cursor = Cursor::new(&args_tokens);
    let mut parameters = vec![];
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
            Some(parse_primitive(return_tokens, trait_name).into())
        }
        _ => None,
    };
    Some(TyFn(Some(parameters), return_type).into())
}

/// 前缀修饰符解析：`&`/`&mut`/`*const`/`*mut`/`self`/`unsafe`
/// （`fn` 由 `parse_function` 或 parse.rs 的裸 `fn` 分支处理）
pub(crate) fn parse_prefix(tokens: &[TokenTree]) -> Option<(TyPrefix, &[TokenTree])> {
    match tokens {
        [TokenTree::Punct(p), TokenTree::Ident(name), rest @ ..]
            if p.as_char() == '&' && name == "mut" =>
        {
            Some((TyPrefix::RefMut, rest))
        }
        [TokenTree::Punct(p), rest @ ..] if p.as_char() == '&' => {
            Some((TyPrefix::Ref, rest))
        }
        [TokenTree::Punct(p), TokenTree::Ident(name), rest @ ..]
            if p.as_char() == '*' && name == "const" =>
        {
            Some((TyPrefix::PtrConst, rest))
        }
        [TokenTree::Punct(p), TokenTree::Ident(name), rest @ ..]
            if p.as_char() == '*' && name == "mut" =>
        {
            Some((TyPrefix::PtrMut, rest))
        }
        [TokenTree::Ident(name), rest @ ..] if name == "self" => {
            Some((TyPrefix::SelfType, rest))
        }
        [TokenTree::Ident(name), rest @ ..] if name == "unsafe" => {
            Some((TyPrefix::Unsafe, rest))
        }
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
    let start = start.to_string().parse::<usize>().ok()?;
    let (inclusive, end) = match rest {
        [TokenTree::Literal(end)] => (false, end),
        [TokenTree::Punct(eq), TokenTree::Literal(end)]
            if eq.as_char() == '=' && second_dot.spacing() == Spacing::Joint =>
        {
            (true, end)
        }
        _ => return None,
    };
    Some(TyRange { start, end: end.to_string().parse().ok()?, inclusive }.into())
}

/// 分组解析：`(A,B)` 元组 / `(A)` 分组 / `[A,B]` 列表 / `[A; N]` 定长数组 / `[A]` 切片 / `{...}` 代码块
pub(crate) fn parse_group(
    group: &proc_macro2::Group, trait_name: Option<&Ident>,
) -> Ty {
    let contents = group.stream().into_iter().collect::<Vec<_>>();
    match group.delimiter() {
        Delimiter::Parenthesis => {
            if contents.is_empty() || contains_punct(&contents, ',') {
                TyTuple(parse_list(&contents, Op::Comma, trait_name)).into()
            } else {
                TyGroup(Box::new(
                    parse_item(&mut Cursor::new(&contents), Op::Dash, trait_name)
                        .unwrap_or_else(empty),
                ))
                .into()
            }
        }
        Delimiter::Bracket => {
            // 有逗号是并列列表；否则以 `;`（Op::Semi）区分定长数组与切片。
            // 空 `[]` 是数组/切片 builder 基座 `(None, None)`。
            if contains_punct(&contents, ',') {
                Ty::Array(TyArray(parse_list(&contents, Op::Comma, trait_name)))
            } else if contents.is_empty() {
                TyPrimitiveArray(None, None).into()
            } else {
                let mut cursor = Cursor::new(&contents);
                let element = parse_item(&mut cursor, Op::Semi, trait_name)
                    .unwrap_or_else(empty);
                if cursor.is_punct(';') {
                    cursor.bump();
                    let length: TokenStream =
                        cursor.take_rest().iter().cloned().collect();
                    TyPrimitiveArray(Some(element.into()), Some(length)).into()
                } else {
                    TyPrimitiveArray(Some(element.into()), None).into()
                }
            }
        }
        Delimiter::Brace => TyWithCode(None, TyCodeBlock(group.stream())).into(),
        _ => empty(),
    }
}

/// 按给定优先级循环解析列表（`parse_item` 返回 None 时停止）
pub(crate) fn parse_list(
    tokens: &[TokenTree], level: Op, trait_name: Option<&Ident>,
) -> Vec<Ty> {
    let mut cursor = Cursor::new(tokens);
    let mut items = vec![];
    while let Some(item) = parse_item(&mut cursor, level, trait_name) {
        items.push(item);
    }
    items
}
