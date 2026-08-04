//! 泛型与尖括号解析模块。
//!
//! 提供 `<...>` 泛型参数的匹配、解析与相关辅助函数。

use proc_macro2::{Ident, TokenStream, TokenTree};
use quote::quote;

use crate::ast::*;
use crate::parse::parse_item;
use crate::scan::{Cursor, is_single_colon, scan_stop};

// ============================================================
// 尖括号与泛型参数
// ============================================================

/// 在 base 后找尖括号组（`delimiter![<>]`，由 `angle_collect` 配对产生），
/// 返回 (base, args, rest)。base 不能为空（空 = 类型参数列表，走 [`parse_type_params`]）。
pub(crate) fn parse_generic(
    tokens: &[TokenTree],
) -> Option<(Vec<TokenTree>, TokenStream, Vec<TokenTree>)> {
    for (i, token) in tokens.iter().enumerate() {
        if let TokenTree::Group(g) = token
            && g.delimiter() == delimiter![<>]
        {
            if i == 0 {
                return None;
            }
            return Some((
                tokens[..i].to_vec(),
                g.stream(),
                tokens[i + 1..].to_vec(),
            ));
        }
    }
    None
}

/// 以尖括号组开头的裸泛型参数列表解析（`<'a, T: Clone>`）。
pub(crate) fn parse_type_params(
    tokens: &[TokenTree],
) -> Option<(TokenStream, Vec<TokenTree>)> {
    let TokenTree::Group(g) = tokens.first()? else {
        return None;
    };
    if g.delimiter() != delimiter![<>] {
        return None;
    }
    Some((g.stream(), tokens[1..].to_vec()))
}

/// 判断 base 是否与 trait_name 重名（用于区分 `TraitName<T>` 与普通泛型）
pub(crate) fn is_trait_base(base: &[TokenTree], trait_name: Option<&Ident>) -> bool {
    trait_name.is_some_and(
        |name| matches!(base.last(), Some(TokenTree::Ident(last)) if last == name),
    )
}

/// 按 separator 切分（尖括号已配对为不透明组，仅按扁平 token 切）
///
/// **注意**：配对组内宏生成的尖括号内容必须保持配对形态——扁平 `<A, B>`
/// 会在逗号处被错误切分（`T: Two<A, B>` → `T: Two<A` / `B>`）。0.6.0 曾有
/// 此缺陷（blanket 的 `T: Trait<X>` bound 实参扁平，靠渲染幂等侥幸正确，
/// dev-changelog F4），已修复为实参组化（preprocess/mod.rs `t_bound`）；
/// 未来宏生成泛型组内容时若含尖括号，须先配对（`Group::new(delimiter![<>], ...)`）
/// 再插入，不得散播扁平 `<...>`。
fn split_at_depth0(tokens: &[TokenTree], separator: char) -> Vec<&[TokenTree]> {
    let mut chunks = vec![];
    let mut rest = tokens;
    while let Some(index) = scan_stop(rest, &[separator]) {
        chunks.push(&rest[..index]);
        rest = &rest[index + 1..];
    }
    chunks.push(rest);
    chunks
}

/// 找到第一个 `:` 且不是 `::` 的位置（用于 `T: Bound` 切分）
fn find_colon_at_depth0(tokens: &[TokenTree]) -> Option<usize> {
    scan_stop(tokens, &[':']).filter(|&index| is_single_colon(tokens, index))
}

/// 解析 `<T: Clone, U, Item=V>` 泛型参数内容：参数列表 + 关联类型绑定
pub(crate) fn parse_angle_bracket_contents(
    tokens: &[TokenTree], trait_name: Option<&Ident>,
) -> TyTypeParam {
    let mut params = vec![];
    let mut bindings = vec![];
    for chunk in split_at_depth0(tokens, ',') {
        if chunk.is_empty() {
            continue;
        }
        if let Some(eq) = scan_stop(chunk, &['=']) {
            bindings.push((
                chunk[..eq].iter().cloned().collect(),
                chunk[eq + 1..].iter().cloned().collect(),
            ));
        } else if let Some(colon) = find_colon_at_depth0(chunk) {
            params.push((
                chunk[..colon].iter().cloned().collect(),
                parse_item(
                    &mut Cursor::new(&chunk[colon + 1..]),
                    Op::Dash,
                    trait_name,
                )
                .unwrap_or_else(empty)
                .into(),
            ));
        } else {
            params.push((chunk.iter().cloned().collect(), None));
        }
    }
    TyTypeParam { params, bindings }
}

// ============================================================
// 兜底
// ============================================================

/// 将 token 序列包装为 Primitive 透传节点（无法识别的类型都走这里）
pub(crate) fn primitive(tokens: &[TokenTree]) -> Ty {
    TyPrimitive(tokens.iter().cloned().collect()).into()
}

/// 空 token 节点（用于 unwrap_or_else 的兜底）
pub(crate) fn empty() -> Ty {
    TyPrimitive(quote![]).into()
}
