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
use crate::generic::{
    is_trait_base, parse_angle_bracket_contents, parse_generic, parse_type_params,
    primitive,
};
use crate::parse_atom::{
    parse_attribute, parse_function, parse_group, parse_prefix, parse_range,
};
use crate::scan::{Cursor, is_punct};
use crate::types::*;

// ============================================================
// 运算符层级解析
// ============================================================

/// 在 `level` 优先级解析一个表达式；遇到更低优先级的运算符停止（留给调用方）。
/// `Op::Semi` / `Op::Comma` 只返回第一个非空项，分隔符之后的部分由调用方继续遍历；
/// Semi 停在 `;` 前且不消费，供 batch_trait! 判断段落边界。
pub(crate) fn parse_item(
    cursor: &mut Cursor, level: Op, trait_name: Option<&Ident>,
) -> Option<Ty> {
    match level {
        Op::Semi | Op::Comma => loop {
            if let Some(item) = parse_operand(cursor, level, trait_name) {
                return item.into();
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
                result = result.apply(parse_operand(cursor, Op::Dash, trait_name)?);
            }
            result.into()
        }
        Op::Caret => {
            let mut items = vec![parse_operand(cursor, Op::Caret, trait_name)?];
            while cursor.is_punct('^') {
                cursor.bump();
                items.push(parse_operand(cursor, Op::Caret, trait_name)?);
            }
            let mut result = items.pop()?;
            while let Some(left) = items.pop() {
                result = left.apply(result);
            }
            result.into()
        }
        Op::Prim => parse_primitive(cursor.take_rest(), trait_name).into(),
    }
}

/// 在 `level` 优先级解析一个操作数（到该层级的停止符为止，停止符不消费）。
///
/// 操作数边界由 `scan_stop` 确定（只看 `<>` 深度，不理解 Rust 类型文法），
/// 边界内的切片交给 `parse_item` 以更高优先级递归解析。
fn parse_operand(
    cursor: &mut Cursor, level: Op, trait_name: Option<&Ident>,
) -> Option<Ty> {
    if cursor.at_end() {
        return None;
    }
    let segment = cursor.take_segment(level.stop_chars());
    parse_item(&mut Cursor::new(segment), level.next()?, trait_name)
}

/// DSL 解析入口：剥离尾部 `{...}` 代码块 / `where{...}` 后缀，
/// 通过 apply 附着到剩余部分解析出的类型上（递归支持连续附着）
pub(crate) fn parse_primitive(
    tokens: &[TokenTree], trait_name: Option<&Ident>,
) -> Ty {
    let split = split_trailing_body(tokens);
    match (split.body, split.is_where) {
        (Some(body), false) => Ty::WithCode(TyWithCode(None, TyCodeBlock(body)))
            .apply(parse_primitive(split.tokens, trait_name)),
        (Some(w), true) => Ty::WithWhere(TyWithWhere(None, TyWhere(w)))
            .apply(parse_primitive(split.tokens, trait_name)),
        _ => parse_primary(split.tokens, trait_name),
    }
}

// ============================================================
// 原子层解析
// ============================================================

/// 尾部 `{...}` 剥离的结果
struct TrailingBody<'a> {
    /// 剥离尾部代码块后的剩余 token
    tokens: &'a [TokenTree],
    /// 剥离出的 body 内容；`None` 表示无尾部代码块
    body: Option<TokenStream>,
    /// `true` 表示 body 是 `where{...}` 谓词后缀
    is_where: bool,
}

/// 分离尾部 `{...}` 代码块（`macro!{...}` 不是尾部代码块；`where{...}` 记为谓词）
fn split_trailing_body(tokens: &[TokenTree]) -> TrailingBody<'_> {
    match tokens.last() {
        Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Brace => {
            // macro!{...} 不是尾部代码块，排除
            if tokens.len() >= 2
                && let TokenTree::Punct(p) = &tokens[tokens.len() - 2]
                && p.as_char() == '!'
            {
                return TrailingBody { tokens, body: None, is_where: false };
            }
            if tokens.len() >= 2
                && let TokenTree::Ident(i) = &tokens[tokens.len() - 2]
                && *i == "where"
            {
                return TrailingBody {
                    tokens: &tokens[..tokens.len() - 2],
                    body: group.stream().into(),
                    is_where: true,
                };
            }
            TrailingBody {
                tokens: &tokens[..tokens.len() - 1],
                body: group.stream().into(),
                is_where: false,
            }
        }
        _ => TrailingBody { tokens, body: None, is_where: false },
    }
}

/// 解析一个"原子"表达式：属性 → 函数 → 前缀 → 范围 → 数字 → 分组 → 泛型 → 类型参数 → 透传兜底
fn parse_primary(tokens: &[TokenTree], trait_name: Option<&Ident>) -> Ty {
    if let Some((attr, rest)) = parse_attribute(tokens) {
        let inner = if rest.is_empty() {
            TyWithAttr(TyAttr(attr), None).into()
        } else {
            // 必须 Ty 包裹走顶层数组分发：`#[attr] [A, B]` => `#[attr] A + #[attr] B`
            Ty::WithAttr(TyWithAttr(TyAttr(attr), None))
                .apply(parse_primitive(rest, trait_name))
        };
        return inner;
    }

    if let Some(function) = parse_function(tokens, trait_name) {
        return function;
    }

    // 裸 `fn`（无参数）：`fn^(A,B)` 由 `^` 操作符后续填入参数
    if let [TokenTree::Ident(name)] = tokens
        && name == "fn"
    {
        return TyFn(None, None).into();
    }

    if let Some((prefix, rest)) = parse_prefix(tokens) {
        let inner = if rest.is_empty() {
            TyWithPrefix(prefix, None).into()
        } else {
            // 必须 Ty 包裹走顶层数组分发：`& [A, B]` => `&A + &B`
            Ty::WithPrefix(TyWithPrefix(prefix, None))
                .apply(parse_primitive(rest, trait_name))
        };
        return inner;
    }

    if let Some(range) = parse_range(tokens) {
        return range;
    }

    if let [TokenTree::Literal(literal)] = tokens
        && let Ok(number) = literal.to_string().parse()
    {
        return TyNum(number).into();
    }

    if let [TokenTree::Group(group)] = tokens {
        return parse_group(group, trait_name);
    }

    if let Some((base, args, rest)) = parse_generic(tokens) {
        let params = parse_angle_bracket_contents(args, trait_name);
        let generic = if is_trait_base(base, trait_name) {
            TyTrait(base.iter().cloned().collect(), params).into()
        } else {
            if !rest.is_empty()
                && !matches!(rest.first(), Some(t) if is_punct(t, '<'))
            {
                return primitive(tokens);
            }
            TyGeneric(primitive(base).into(), params).into()
        };
        return if rest.is_empty() {
            generic
        } else {
            generic.apply(parse_primitive(rest, trait_name))
        };
    }

    if let Some((args, rest)) = parse_type_params(tokens) {
        let params = parse_angle_bracket_contents(args, trait_name);
        let params = params.into();
        return if rest.is_empty() {
            params
        } else {
            params.apply(parse_primitive(rest, trait_name))
        };
    }

    primitive(tokens)
}
