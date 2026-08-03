//! 扫描与游标模块。
//!
//! 提供轻量 [`Cursor`]（`&[TokenTree]` 借用切片游标）和统一停止符扫描
//! [`scan_with`] / [`scan_stop`]。尖括号已由 `angle_collect` 配对为
//! 不透明组，扫描不再跟踪 `<>` 深度；唯一保留的守卫是 `->` 箭头
//! （`-` 后接 `>` 时 `-` 不是 Dash 停止符）。

use proc_macro2::{Spacing, TokenTree};

// ============================================================
// 游标与统一扫描原语
// ============================================================

/// 借用 token 切片的轻量游标，按顺序向前消费。
///
/// parse 层的核心数据结构：所有 DSL 解析函数围绕游标推进，
/// 消费模型是"扫描到停止符、取切片、递归解析"。
pub(crate) struct Cursor<'a> {
    tokens: &'a [TokenTree],
    pos: usize,
}

impl<'a> Cursor<'a> {
    pub(crate) fn new(tokens: &'a [TokenTree]) -> Self {
        Self { tokens, pos: 0 }
    }

    pub(crate) fn at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    pub(crate) fn peek(&self) -> Option<&'a TokenTree> {
        self.tokens.get(self.pos)
    }

    pub(crate) fn peek_at(&self, offset: usize) -> Option<&'a TokenTree> {
        self.tokens.get(self.pos + offset)
    }

    pub(crate) fn is_punct(&self, ch: char) -> bool {
        matches!(self.tokens.get(self.pos), Some(t) if is_punct(t, ch))
    }

    /// 当前位置的前一个 token 是否为指定标点（用于识别 `ident!` 宏调用体）
    pub(crate) fn prev_is_punct(&self, ch: char) -> bool {
        self.pos > 0
            && matches!(self.tokens.get(self.pos - 1), Some(t) if is_punct(t, ch))
    }

    /// 当前位置的 `:` 是否为独立单冒号（非 `::` 的组成部分）
    pub(crate) fn is_single_colon(&self) -> bool {
        is_single_colon(self.tokens, self.pos)
    }

    pub(crate) fn bump(&mut self) {
        self.pos += 1;
    }

    pub(crate) fn pos(&self) -> usize {
        self.pos
    }

    /// 取出从 start 到当前位置的切片
    pub(crate) fn slice_since(&self, start: usize) -> &'a [TokenTree] {
        &self.tokens[start..self.pos]
    }

    /// 取出直到下一个 depth-0 停止符的切片（停止符留在标中，不消费）
    pub(crate) fn take_segment(&mut self, stop: &[char]) -> &'a [TokenTree] {
        let tokens = self.tokens;
        let rest = &tokens[self.pos..];
        let end = scan_stop(rest, stop).unwrap_or(rest.len());
        self.pos += end;
        &rest[..end]
    }

    /// 取出剩余全部
    pub(crate) fn take_rest(&mut self) -> &'a [TokenTree] {
        let tokens = self.tokens;
        let rest = &tokens[self.pos..];
        self.pos = tokens.len();
        rest
    }
}

/// 统一的停止符扫描：返回第一个 depth-0 且属于 stop 集合的 token 索引。
///
/// 尖括号已由 `angle_collect` 配对为组（不透明），此处不再跟踪 `<>` 深度；
/// 唯一保留的守卫是 `->` 箭头：`-` 后接 `>` 时 `-` 不是 Dash 停止符。
pub(crate) fn scan_with(tokens: &[TokenTree], stop: &[char]) -> Option<usize> {
    for (index, token) in tokens.iter().enumerate() {
        if matches!(token, TokenTree::Punct(p) if stop.contains(&p.as_char())) {
            let is_arrow_dash = matches!(token, TokenTree::Punct(p)
                if p.as_char() == '-' && p.spacing() == Spacing::Joint)
                && matches!(tokens.get(index + 1), Some(next) if is_punct(next, '>'));
            if !is_arrow_dash {
                return index.into();
            }
        }
    }
    None
}

/// 返回第一个 depth-0 且属于 stop 集合的 token 索引。
pub(crate) fn scan_stop(tokens: &[TokenTree], stop: &[char]) -> Option<usize> {
    scan_with(tokens, stop)
}

/// 判断单个 token 是否为指定标点符号
pub(crate) fn is_punct(token: &TokenTree, punctuation: char) -> bool {
    matches!(token, TokenTree::Punct(p) if p.as_char() == punctuation)
}

/// 判断 `tokens[index]` 是否为 `->` 的 `>`（前一个 token 是 Joint 的 `-`）。
///
/// `->` 箭头在扫描中不作为 `>` 深度计数，也不作为 DSL 停止符。
pub(crate) fn is_arrow(tokens: &[TokenTree], index: usize) -> bool {
    index > 0
        && matches!(&tokens[index - 1], TokenTree::Punct(p)
            if p.as_char() == '-' && p.spacing() == Spacing::Joint)
}

/// 判断 `tokens[index]` 是否为独立的 `:`（不是 `::` 的组成部分）。
///
/// `::` 的两个 `:` 中前一个 `Spacing::Joint`（紧跟后一个），据此排除：
/// 前一个 token 是 Joint 的 `:`（本 token 是 `::` 的后半），或
/// 后一个 token 是 `:` 且本 token `Spacing::Joint`（本 token 是 `::` 的前半）。
pub(crate) fn is_single_colon(tokens: &[TokenTree], index: usize) -> bool {
    let Some(TokenTree::Punct(p)) = tokens.get(index) else {
        return false;
    };
    p.as_char() == ':'
        && !(index > 0
            && matches!(&tokens[index - 1], TokenTree::Punct(q)
                if q.as_char() == ':' && q.spacing() == Spacing::Joint)
            || index + 1 < tokens.len()
                && matches!(&tokens[index + 1], TokenTree::Punct(q)
                    if q.as_char() == ':' && p.spacing() == Spacing::Joint))
}

/// 判断 token 序列中是否包含指定的顶层标点符号
pub(crate) fn contains_punct(tokens: &[TokenTree], punctuation: char) -> bool {
    tokens.iter().any(|token| is_punct(token, punctuation))
}
