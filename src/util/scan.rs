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

    pub(crate) fn peek(&self) -> Option<&'a TokenTree> {
        self.tokens.get(self.pos)
    }

    pub(crate) fn peek_at(&self, offset: usize) -> Option<&'a TokenTree> {
        self.tokens.get(self.pos + offset)
    }

    pub(crate) fn is_punct(&self, ch: char) -> bool {
        matches!(self.tokens.get(self.pos), Some(t) if is_punct(t, ch))
    }

    /// Whether the Bracket group at the current position passes through (the previous token
    /// is `!` or `#`); synonymous with [`bracket_is_passthrough`], for cursor-style traversal.
    pub(crate) fn prev_bracket_passthrough(&self) -> bool {
        bracket_is_passthrough(self.tokens, self.pos)
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

/// Return the index of the first depth-0 token in the stop set.
pub(crate) fn scan_stop(tokens: &[TokenTree], stop: &[char]) -> Option<usize> {
    scan_with(tokens, stop)
}

/// Check whether a single token is the given punctuation
pub(crate) fn is_punct(token: &TokenTree, punctuation: char) -> bool {
    matches!(token, TokenTree::Punct(p) if p.as_char() == punctuation)
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
