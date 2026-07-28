//! 扫描与游标模块。
//!
//! 提供轻量 [`Cursor`]（`&[TokenTree]` 借用切片游标）和统一扫描原语
//! [`scan_with`] / [`ScanMode`]。深度跟踪统一通过 `scan_with` 完成；
//! `scan_stop`（宽松）与 `matching_angle`（严格，见 `generic` 模块）
//! 是其两个对外语义别名，行为同旧版本但共用同一份循环。

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

/// `<>` 深度扫描模式：Lossy 宽松（饱和减）、Strict 严格（失衡返回 None）。
pub(crate) enum ScanMode {
    Lossy,
    Strict,
}

/// 统一的 `<>` 深度扫描：返回第一个 depth-0 且属于 stop 集合的 token 索引。
///
/// - `->` 的 `>` 不计深度；`-` 后接 `>` 是箭头而非停止符。
/// - Lossy：遇到停止符或失衡也用饱和减忽略（用于 `scan_stop`）。
/// - Strict：尖括号严格配对，失衡返回 None（用于 `matching_angle`）。
pub(crate) fn scan_with(
    tokens: &[TokenTree],
    stop: &[char],
    mode: ScanMode,
) -> Option<usize> {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate() {
        if is_punct(token, '<') {
            depth += 1;
        } else if is_punct(token, '>')
            && !(index > 0
                && matches!(&tokens[index - 1], TokenTree::Punct(p) if p.as_char() == '-' && p.spacing() == Spacing::Joint))
        {
            match mode {
                ScanMode::Lossy => depth = depth.saturating_sub(1),
                ScanMode::Strict => {
                    depth = depth.checked_sub(1)?;
                    if depth == 0 {
                        return Some(index);
                    }
                },
            }
        } else if depth == 0
            && matches!(token, TokenTree::Punct(p) if stop.contains(&p.as_char()))
        {
            let is_arrow_dash = matches!(&tokens.get(index), Some(TokenTree::Punct(p)) if p.as_char() == '-' && p.spacing() == Spacing::Joint)
                && matches!(tokens.get(index + 1), Some(next) if is_punct(next, '>'));
            if !is_arrow_dash {
                return Some(index);
            }
        }
    }
    None
}

/// 宽松版：返回第一个 depth-0 且属于 stop 集合的 token 索引（失衡忽略）。
pub(crate) fn scan_stop(tokens: &[TokenTree], stop: &[char]) -> Option<usize> {
    scan_with(tokens, stop, ScanMode::Lossy)
}

/// 判断单个 token 是否为指定标点符号
pub(crate) fn is_punct(token: &TokenTree, punctuation: char) -> bool {
    matches!(token, TokenTree::Punct(p) if p.as_char() == punctuation)
}

/// 判断 token 序列中是否包含指定的顶层标点符号
pub(crate) fn contains_punct(tokens: &[TokenTree], punctuation: char) -> bool {
    tokens.iter().any(|token| is_punct(token, punctuation))
}
