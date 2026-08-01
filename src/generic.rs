//! 泛型与尖括号解析模块。
//!
//! 提供 `<...>` 泛型参数的匹配、解析与相关辅助函数。

use proc_macro2::{Ident, TokenTree};
use quote::quote;

use crate::parse::parse_item;
use crate::scan::{
    Cursor, ScanMode, is_punct, is_single_colon, scan_stop, scan_with,
};
use crate::types::*;

// ============================================================
// 尖括号与泛型参数
// ============================================================

/// 在 base 后找 `<...>` 泛型参数（base 不能为空，返回 (base, args, rest)）
pub(crate) fn parse_generic(
    tokens: &[TokenTree],
) -> Option<(&[TokenTree], &[TokenTree], &[TokenTree])> {
    // 找第一个 `<`
    let mut i = 0usize;
    let mut open = None;
    while i < tokens.len() {
        if is_punct(&tokens[i], '<') {
            open = i.into();
            break;
        }
        i += 1;
    }
    let open = open?;
    if open == 0 {
        return None;
    }
    let close = matching_angle(tokens, open)?;
    (
        &tokens[..open],
        &tokens[open + 1..close],
        &tokens[close + 1..],
    )
        .into()
}

/// 以 `<` 开头的裸泛型参数列表解析
pub(crate) fn parse_type_params(
    tokens: &[TokenTree],
) -> Option<(&[TokenTree], &[TokenTree])> {
    if !matches!(tokens.first(), Some(token) if is_punct(token, '<')) {
        return None;
    }
    let close = matching_angle(tokens, 0)?;
    (&tokens[1..close], &tokens[close + 1..]).into()
}

/// 严格配对：找到 open 处 `<` 对应的 `>`；深度失衡返回 None。
///
/// 基于 `scan_with(Strict)` 实现：截取 `tokens[open..]` 后扫描，
/// 找到的索引加回 `open` 还原到原 token 序列的位置。
pub(crate) fn matching_angle(tokens: &[TokenTree], open: usize) -> Option<usize> {
    let sub = &tokens[open..];
    scan_with(sub, &[], ScanMode::Strict).map(|i| i + open)
}

/// 判断 base 是否与 trait_name 重名（用于区分 `TraitName<T>` 与普通泛型）
pub(crate) fn is_trait_base(base: &[TokenTree], trait_name: Option<&Ident>) -> bool {
    trait_name.is_some_and(
        |name| matches!(base.last(), Some(TokenTree::Ident(last)) if last == name),
    )
}

/// 在 depth-0 按 separator 切分（`->` 中的 `>` 不改变深度）
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

/// 找到第一个 depth-0 的 `:` 且不是 `::` 的位置（用于 `T: Bound` 切分）
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
