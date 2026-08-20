//! Scanning and cursor module.
//!
//! Provides a lightweight [`Cursor`] (a borrowed-slice cursor over `&[TokenTree]`) and the
//! unified stop-token scanners [`scan_with`] / [`scan_stop`]. Angle brackets were paired into
//! opaque groups by `angle_collect`, so scanning no longer tracks `<>` depth; the only
//! remaining guard is the `->` arrow (`-` is not a Dash stop token when followed by `>`).

use proc_macro2::{Spacing, TokenTree};

// ============================================================
// Cursor and unified scanning primitives
// ============================================================

/// Lightweight cursor borrowing a token slice, consuming forward in order.
///
/// The core data structure of the parse layer: every DSL parsing function advances around
/// the cursor, with a "scan to a stop token, take a slice, recurse" consumption model.
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

    /// Span at the current position (call_site when at end).
    pub(crate) fn span(&self) -> proc_macro2::Span {
        self.peek().map(|t| t.span()).unwrap_or_else(proc_macro2::Span::call_site)
    }

    pub(crate) fn peek(&self) -> Option<&'a TokenTree> {
        self.tokens.get(self.pos)
    }

    /// Token at an offset from the current position (bounds-safe).
    pub(crate) fn peek_at(&self, off: usize) -> Option<&'a TokenTree> {
        self.tokens.get(self.pos + off)
    }

    /// Advance by `n` tokens (clamped to the end).
    pub(crate) fn advance(&mut self, n: usize) {
        self.pos = (self.pos + n).min(self.tokens.len());
    }

    /// The `n` tokens starting at absolute index `start` (independent of the
    /// cursor position — used to collect a block's token extent).
    pub(crate) fn slice_at(&self, start: usize, n: usize) -> &'a [TokenTree] {
        let end = start.saturating_add(n).min(self.tokens.len());
        &self.tokens[start.min(self.tokens.len())..end]
    }

    pub(crate) fn is_punct(&self, ch: char) -> bool {
        is_punct_at(self.tokens, self.pos, ch)
    }

    /// Whether the `:` at the current position is a standalone single colon (not part of `::`)
    pub(crate) fn is_single_colon(&self) -> bool {
        is_single_colon(self.tokens, self.pos)
    }

    pub(crate) fn bump(&mut self) {
        self.pos += 1;
    }

    pub(crate) fn pos(&self) -> usize {
        self.pos
    }

    /// Take the slice from start to the current position
    pub(crate) fn slice_since(&self, start: usize) -> &'a [TokenTree] {
        &self.tokens[start..self.pos]
    }

    /// Take the slice up to the next depth-0 stop token (the stop token stays in the
    /// cursor, unconsumed)
    pub(crate) fn take_segment(&mut self, stop: &[char]) -> &'a [TokenTree] {
        let tokens = self.tokens;
        let rest = &tokens[self.pos..];
        let end = scan_stop(rest, stop).unwrap_or(rest.len());
        self.pos += end;
        &rest[..end]
    }

    /// Take everything remaining
    pub(crate) fn take_rest(&mut self) -> &'a [TokenTree] {
        let tokens = self.tokens;
        let rest = &tokens[self.pos..];
        self.pos = tokens.len();
        rest
    }
}

/// Unified stop-token scan: return the index of the first depth-0 token in the stop set.
///
/// Angle brackets were paired into opaque groups by `angle_collect`, so `<>` depth is not
/// tracked here; the only guard kept is the `->` arrow: `-` is not a Dash stop token when
/// followed by `>`.
pub(crate) fn scan_stop(tokens: &[TokenTree], stop: &[char]) -> Option<usize> {
    for (index, token) in tokens.iter().enumerate() {
        if matches!(token, TokenTree::Punct(p) if stop.contains(&p.as_char())) {
            let is_arrow_dash = is_joint_punct_at(tokens, index, '-')
                && matches!(tokens.get(index + 1), Some(next) if is_punct(next, '>'));
            // `..=` inclusive range: the second `=` is part of the range
            // operator, not a binding separator (`Item = u32`).
            let is_range_inclusive = is_punct(token, '=')
                && index > 0
                && matches!(tokens.get(index - 1), Some(prev) if is_punct(prev, '.'));
            // `.` as an apply operator must not cut a `..` range (`1..=4` /
            // `@1..` — the two dots stay one unit, parsed by `parse_range`).
            let is_dot_range = is_punct(token, '.')
                && (index > 0
                    && matches!(tokens.get(index - 1), Some(prev) if is_punct(prev, '.'))
                    || matches!(tokens.get(index + 1), Some(next) if is_punct(next, '.')));
            if !is_arrow_dash && !is_range_inclusive && !is_dot_range {
                return index.into();
            }
        }
    }
    None
}

/// Check whether a single token is the given punctuation
pub(crate) fn is_punct(token: &TokenTree, punctuation: char) -> bool {
    matches!(token, TokenTree::Punct(p) if p.as_char() == punctuation)
}

/// Whether `tokens[index]` is the given punctuation (bounds-safe).
pub(crate) fn is_punct_at(tokens: &[TokenTree], index: usize, ch: char) -> bool {
    tokens.get(index).is_some_and(|t| is_punct(t, ch))
}

/// Whether the token is the given punctuation with `Joint` spacing (e.g. the
/// first `.` of `..`, the `-` of `->`, the first `:` of `::`).
pub(crate) fn is_joint_punct_at(tokens: &[TokenTree], index: usize, ch: char) -> bool {
    matches!(tokens.get(index), Some(TokenTree::Punct(p))
        if p.as_char() == ch && p.spacing() == Spacing::Joint)
}

/// Whether a Bracket group (`[...]`) "passes through": when the previous token is `!`
/// (an `ident![...]` macro call body) or `#` (a `#[...]` attribute), the group may contain
/// arbitrary Rust (comparison `<`, `#name` directives, etc.); the recursive entry
/// points (`angle_collect` / `expand_consts` / `expand_tokens` / `where_process`) decide
/// uniformly, to avoid guard drift (0.5.7 mis-expanded `#name` due to a missing `#[...]` guard).
pub(crate) fn bracket_is_passthrough(tokens: &[TokenTree], index: usize) -> bool {
    index > 0
        && matches!(&tokens[index - 1], TokenTree::Punct(p)
            if p.as_char() == '!' || p.as_char() == '#')
}

/// Whether `tokens[index]` is the `impl` ident of an `impl{...}` shape
/// template: an `impl` ident directly followed by a Brace
/// group. The single authority for the `impl{...}` discrimination shared by
/// `expand_consts` (which enters the template to expand `@`/`@trait`) and
/// `where_process` (which treats it as a predicate-region boundary).
pub(crate) fn is_impl_template(tokens: &[TokenTree], index: usize) -> bool {
    matches!(
        (tokens.get(index), tokens.get(index + 1)),
        (
            Some(TokenTree::Ident(id)),
            Some(TokenTree::Group(g)),
        ) if *id == "impl" && g.delimiter() == proc_macro2::Delimiter::Brace
    )
}

/// Check whether `tokens[index]` is the `>` of `->` (previous token is a Joint `-`).
///
/// The `->` arrow neither counts as `>` depth in scanning nor acts as a DSL stop token.
pub(crate) fn is_arrow(tokens: &[TokenTree], index: usize) -> bool {
    index > 0
        && matches!(&tokens[index - 1], TokenTree::Punct(p)
            if p.as_char() == '-' && p.spacing() == Spacing::Joint)
}

/// Check whether `tokens[index]` is a standalone `:` (not part of `::`).
///
/// In `::`, the first `:` has `Spacing::Joint` (directly followed by the second), which
/// rules out: the previous token being a Joint `:` (this token is the second half of `::`),
/// or the next token being `:` while this token is `Spacing::Joint` (this token is the
/// first half of `::`).
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

/// Check whether the token sequence contains the given top-level punctuation
pub(crate) fn contains_punct(tokens: &[TokenTree], punctuation: char) -> bool {
    tokens.iter().any(|token| is_punct(token, punctuation))
}
