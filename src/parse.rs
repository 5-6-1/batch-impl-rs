//! DSL 解析器。
//!
//! 接受 `Cursor`（`&[TokenTree]` 借用切片游标），按四级优先级攀爬
//! `Op::Semi` < `Op::Comma` < `Op::Dash` < `Op::Caret` < `Op::Prim`
//! 解析为 `Ty` AST。
//!
//! 依赖 [`crate::scan`] 模块提供游标与扫描原语，
//! 依赖 [`crate::generic`] 模块提供泛型与尖括号解析。

use proc_macro2::{Delimiter, Ident, TokenStream, TokenTree};

use crate::apply::Apply;
use crate::generic::{is_trait_base, parse_angle_bracket_contents, parse_generic, parse_type_params, primitive};
use crate::parse_atom::{parse_attribute, parse_function, parse_group, parse_prefix, parse_range};
use crate::scan::{Cursor, is_punct};
use crate::types::*;

// ============================================================
// 运算符层级解析
// ============================================================

/// 在 `level` 优先级解析一个表达式；遇到更低优先级的运算符停止（留给调用方）。
/// `Op::Semi` / `Op::Comma` 只返回第一个非空项，分隔符之后的部分由调用方继续遍历；
/// Semi 停在 `;` 前且不消费，供 batch_trait! 判断段落边界。
pub(crate) fn parse_item(
    cursor: &mut Cursor,
    level: Op,
    trait_name: Option<&Ident>,
) -> Option<Ty> {
    match level {
        Op::Semi | Op::Comma => loop {
            if let Some(item) = parse_operand(cursor, level, trait_name) {
                return Some(item);
            }
            if cursor.is_punct(',') {
                cursor.bump();
            } else {
                return None;
            }
        },
        Op::Dash => {
            let mut result = parse_operand(cursor, Op::Dash, trait_name)?;
            while cursor.is_punct('-') {
                cursor.bump();
                result = result.apply(parse_operand(
                    cursor,
                    Op::Dash,
                    trait_name,
                )?);
            }
            Some(result)
        },
        Op::Caret => {
            let mut items =
                vec![parse_operand(cursor, Op::Caret, trait_name)?];
            while cursor.is_punct('^') {
                cursor.bump();
                items.push(parse_operand(cursor, Op::Caret, trait_name)?);
            }
            let mut result = items.pop()?;
            while let Some(left) = items.pop() {
                result = left.apply(result);
            }
            Some(result)
        },
        Op::Prim => Some(parse_primitive(cursor.take_rest(), trait_name)),
    }
}

/// 在 `level` 优先级解析一个操作数（到该层级的停止符为止，停止符不消费）。
///
/// 操作数边界由 `scan_stop` 确定（只看 `<>` 深度，不理解 Rust 类型文法），
/// 边界内的切片交给 `parse_item` 以更高优先级递归解析。
fn parse_operand(
    cursor: &mut Cursor,
    level: Op,
    trait_name: Option<&Ident>,
) -> Option<Ty> {
    if cursor.at_end() {
        return None;
    }
    let segment = cursor.take_segment(level.stop_chars());
    parse_item(&mut Cursor::new(segment), level.next()?, trait_name)
}

/// DSL 解析入口：剥离尾部 `{...}` 代码块后交给 `attach_body`（递归支持连续附着）
pub(crate) fn parse_primitive(
    tokens: &[TokenTree],
    trait_name: Option<&Ident>,
) -> Ty {
    let (tokens, body,is_where) = split_trailing_body(tokens);
    match (body,is_where) {
        (Some(body),false) => Ty::CodeBlock(TyCodeBlock(body))
            .apply(parse_primitive(tokens, trait_name)),
        (Some(w),true) => Ty::Where(TyWhere(w))
            .apply(parse_primitive(tokens,trait_name)),
        _ => parse_primary(tokens, trait_name),
    }
}

// ============================================================
// 原子层解析
// ============================================================

/// 分离尾部 `{...}` 代码块（`macro!{...}` 不是尾部代码块）
fn split_trailing_body(
    tokens: &[TokenTree],
) -> (&[TokenTree], Option<TokenStream>,bool) {
    match tokens.last() {
        Some(TokenTree::Group(group))
            if group.delimiter() == Delimiter::Brace => {
            // macro!{...} 不是尾部代码块，排除
            if tokens.len() >= 2
                && let TokenTree::Punct(p) = &tokens[tokens.len() - 2]
                && p.as_char() == '!' {
                return (tokens, None,false);
            }
            if tokens.len()>=2 
                && let TokenTree::Ident(i)=&tokens[tokens.len() - 2]
                && i.to_string()=="where"{
                return (&tokens[..tokens.len() - 2], Some(group.stream()),true)
            }
            (&tokens[..tokens.len() - 1], Some(group.stream()),false)
        },
        _ => (tokens, None,false),
    }
}

/// 解析一个"原子"表达式：属性 → 函数 → 前缀 → 范围 → 数字 → 分组 → 泛型 → 类型参数 → 透传兜底
fn parse_primary(tokens: &[TokenTree], trait_name: Option<&Ident>) -> Ty {
    if let Some((attr, rest)) = parse_attribute(tokens) {
        let inner = if rest.is_empty() {
            TyAttr(attr).into()
        } else {
            TyAttr(attr).apply(parse_primitive(rest, trait_name))
        };
        return inner;
    }

    if let Some(function) = parse_function(tokens, trait_name) {
        return function;
    }

    if let Some((prefix, rest)) = parse_prefix(tokens) {
        let inner = if rest.is_empty() {
            Ty::Prefix(prefix)
        } else {
            prefix.apply(parse_primitive(rest, trait_name))
        };
        return inner;
    }

    if let Some(range) = parse_range(tokens) {
        return range;
    }

    if let [TokenTree::Literal(literal)] = tokens
        && let Ok(number) = literal.to_string().parse::<u8>()
    {
        return Ty::Num(TyNum(number));
    }

    if let [TokenTree::Group(group)] = tokens {
        return parse_group(group, trait_name);
    }

    if let Some((base, args, rest)) = parse_generic(tokens) {
        let params = parse_angle_bracket_contents(args, trait_name);
        let generic = if is_trait_base(base, trait_name) {
            Ty::Trait(TyTrait(base.iter().cloned().collect(), params))
        } else {
            if !rest.is_empty()
                && !matches!(rest.first(), Some(t) if is_punct(t, '<'))
            {
                return primitive(tokens);
            }
            Ty::Generic(TyGeneric(Box::new(primitive(base)), params))
        };
        return if rest.is_empty() {
            generic
        } else {
            generic.apply(parse_primitive(rest, trait_name))
        };
    }

    if let Some((args, rest)) = parse_type_params(tokens) {
        let params = parse_angle_bracket_contents(args, trait_name);
        let params = Ty::TypeParam(params);
        return if rest.is_empty() {
            params
        } else {
            params.apply(parse_primitive(rest, trait_name))
        };
    }

    primitive(tokens)
}


