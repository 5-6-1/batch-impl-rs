//! 解析层：DSL 优先级攀爬解析器与尖括号泛型解析。

mod generic;
mod parse_atom;

use proc_macro2::{Ident, TokenStream, TokenTree};

use crate::apply::{Apply, err_ty};
use crate::ast::*;
use crate::parse::generic::{
    is_trait_base, parse_angle_bracket_contents, parse_generic, parse_type_params,
    primitive,
};
use crate::parse::parse_atom::{
    parse_attribute, parse_function, parse_group, parse_prefix, parse_range,
};
use crate::scan::Cursor;

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
                // 连续逗号（`,,`）：两个分隔符之间无操作数。
                // 尾随单逗号合法（由调用方判定 `A,` 结束）；双逗号是笔误。
                if cursor.is_punct(',') {
                    return err_ty(
                        "batch-impl: 连续逗号 `,,` 之间缺少操作数（如 `A,,B`）",
                    )
                    .into();
                }
            } else {
                return None;
            }
        },
        Op::Dash => {
            // 左操作数：parse_operand 返回 None 仅在游标到末尾（合法终止）或
            // 空段（`-A` 左空，静默吞段）。空段必须报错。
            let mut result = match parse_operand(cursor, Op::Dash, trait_name) {
                Some(op) => op,
                None if cursor.at_end() => return None,
                None => {
                    return err_ty("batch-impl: `-` 前缺少操作数（如 `T-U`）").into();
                }
            };
            if is_empty_operand(&result) {
                return err_ty("batch-impl: `-` 前缺少操作数（如 `T-U`）").into();
            }
            while cursor.is_punct('-') {
                cursor.bump();
                let Some(op) = parse_operand(cursor, Op::Dash, trait_name) else {
                    return err_ty("batch-impl: `-` 后缺少操作数（如 `T-U`）").into();
                };
                if is_empty_operand(&op) {
                    return err_ty("batch-impl: `-` 后缺少操作数（如 `T-U`）").into();
                }
                result = result.apply(op);
            }
            result.into()
        }
        Op::Caret => {
            // 左操作数：`^A` 的空段会解析为空 Primitive，必须拦截
            // （否则生成 ` <A>` 垃圾类型，下游报错定位不到 DSL）。
            let first = parse_operand(cursor, Op::Caret, trait_name)?;
            if is_empty_operand(&first) {
                return err_ty("batch-impl: `^` 前缺少操作数（如 `T^U`）").into();
            }
            let mut items = vec![first];
            while cursor.is_punct('^') {
                cursor.bump();
                let Some(op) = parse_operand(cursor, Op::Caret, trait_name) else {
                    return err_ty("batch-impl: `^` 后缺少操作数（如 `T^U`）").into();
                };
                if is_empty_operand(&op) {
                    return err_ty("batch-impl: `^` 后缺少操作数（如 `T^U`）").into();
                }
                items.push(op);
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

/// 操作数是否为空（`^`/`-` 后紧跟深度 0 的停止符时，`take_segment` 会截出空切片）。
/// 空操作数即"运算符后缺操作数"；`()`/`[]` 等 Group 虽可为空元组/空基座，
/// 但它们是一个真实 token，不是空操作数。
fn is_empty_operand(ty: &Ty) -> bool {
    matches!(ty, Ty::Primitive(p) if p.0.is_empty())
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
        (Some(body), false) => {
            let block = TyWithCode(None, TyCodeBlock(body));
            // 整个操作数就是 `{...}` 裸代码块（开放指令独立成 spec 的退化形态）：
            // 保持 `None` 内层作为"顶层 item 注入"标记，不附着空目标
            // （附着到类型时 `T {code}` 是普通 impl body，走下方 apply）
            if split.tokens.is_empty() {
                return block.into();
            }
            block.apply(parse_primitive(split.tokens, trait_name))
        }
        (Some(w), true) => TyWithWhere(None, TyWhere(w))
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
        Some(TokenTree::Group(group)) if group.delimiter() == delimiter![{}] => {
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
            TyWithAttr(TyAttr(attr), None).apply(parse_primitive(rest, trait_name))
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
        return TyFn(None, None, false).into();
    }

    if let Some((prefix, rest)) = parse_prefix(tokens) {
        // `unsafe` 前缀歧义消解：
        // - 裸 `unsafe`（rest 空）→ unsafe impl 标记（unsafe^T / unsafe-T），原样透传
        // - `unsafe fn...` → unsafe fn 类型（TyFn.is_unsafe 置位）
        // - `unsafe X`（X 非 fn）→ 报错：Rust 中 unsafe 只能修饰 fn 类型，
        //   并列写其他类型几乎必是忘写 `^` 的笔误（unsafe^Vec<T>）
        if matches!(prefix, TyPrefix::Unsafe) && !rest.is_empty() {
            if matches!(rest.first(), Some(TokenTree::Ident(f)) if f == "fn") {
                let inner = parse_primitive(rest, trait_name);
                return match inner {
                    Ty::Fn(mut f) => {
                        f.2 = true;
                        f.into()
                    }
                    // rest 以 `fn` 开头，parse_primitive 必得 TyFn；防御性兜底
                    other => other,
                };
            }
            return err_ty(
                "batch-impl: `unsafe` 只能修饰 fn 类型（如 `unsafe fn(u32) -> u32`）\
                 或作为裸 impl 标记（如 `unsafe^T`）",
            );
        }
        let inner = if rest.is_empty() {
            TyWithPrefix(prefix, None).into()
        } else {
            TyWithPrefix(prefix, None).apply(parse_primitive(rest, trait_name))
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
        let args_vec: Vec<_> = args.into_iter().collect();
        let params = parse_angle_bracket_contents(&args_vec, trait_name);
        let generic = if is_trait_base(&base, trait_name) {
            TyTrait(base.iter().cloned().collect(), params).into()
        } else {
            // rest 非空且不是尖括号组（`Vec<T><U>` 是连续泛型，走 apply）：
            // 其他（如 `Vec<T>U`）视为透传
            if !rest.is_empty()
                && !matches!(rest.first(), Some(TokenTree::Group(g)) if g.delimiter() == delimiter![<>])
            {
                return primitive(tokens);
            }
            TyGeneric(primitive(&base).into(), params).into()
        };
        return if rest.is_empty() {
            generic
        } else {
            generic.apply(parse_primitive(&rest, trait_name))
        };
    }

    if let Some((args, rest)) = parse_type_params(tokens) {
        let args_vec: Vec<_> = args.into_iter().collect();
        let params = parse_angle_bracket_contents(&args_vec, trait_name);
        let params = params.into();
        return if rest.is_empty() {
            params
        } else {
            params.apply(parse_primitive(&rest, trait_name))
        };
    }

    primitive(tokens)
}
