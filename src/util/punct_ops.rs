//! The operator dictionary: the single authority for multi-character
//! operator shapes (`..`, `..=`, `->`, `::`) and plain puncts. Callers ask
//! "what operator starts at `i`?" and get the kind plus the length to skip —
//! no caller re-derives `Spacing::Joint` combinations by hand. The shapes
//! mirror what the lexer actually produces:
//!
//! - `..`  = `.`(Joint) `.`
//! - `..=` = `.`(Joint) `.`(Joint) `=`
//! - `->`  = `-`(Joint) `>`
//! - `::`  = `:`(Joint) `:`
//!
//! A lone `.`/`-`/`:` (Alone, or Joint against a non-operator follower such
//! as the `.` of `self.0`) is the plain single-char operator. Compound
//! operators are consumed as one unit everywhere: they are never individual
//! scan stops, never standalone operands.

use proc_macro2::{Spacing, TokenTree};

use crate::util::{is_joint_punct_at, is_punct_at};

/// The DSL operator alphabet, as read off a token stream.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Op {
    /// `.` — the apply operator.
    Dot,
    /// `..` — an open/exclusive range (`@0..`, `1..4`).
    DotDot,
    /// `..=` — an inclusive range (`@0..=M`).
    DotDotEq,
    /// `->` — the fn return arrow.
    Arrow,
    /// `:` — a standalone colon.
    Colon,
    /// `::` — the path separator.
    ColonColon,
    /// `-` — a lone dash (not the head of `->`).
    Minus,
    /// `=` — a binding separator (`Item = u32`).
    Eq,
    /// `,`
    Comma,
    /// `;`
    Semicolon,
    /// `@`
    At,
    /// `!`
    Bang,
    /// `+`
    Plus,
}

impl Op {
    /// The leading character — what a stop-set filter compares against.
    pub(crate) fn first_char(self) -> char {
        match self {
            Op::Dot | Op::DotDot | Op::DotDotEq => '.',
            Op::Arrow | Op::Minus => '-',
            Op::Colon | Op::ColonColon => ':',
            Op::Eq => '=',
            Op::Comma => ',',
            Op::Semicolon => ';',
            Op::At => '@',
            Op::Bang => '!',
            Op::Plus => '+',
        }
    }

    /// Whether the operator spans several tokens (`..` / `..=` / `->` / `::`).
    /// A compound operator's members are consumed as one unit — none of them
    /// can be a scan stop or a standalone operand.
    pub(crate) fn is_compound(self) -> bool {
        matches!(self, Op::DotDot | Op::DotDotEq | Op::Arrow | Op::ColonColon)
    }
}

/// Reads the operator starting at `tokens[i]`: `(op, len)` — `len` is how
/// many tokens the whole operator spans (1 for a plain punct). `None` when
/// `tokens[i]` is not a punct. The multi-char shapes require the head to be
/// `Joint` (the lexer's glued form); a spaced-apart pair (`1 . . 4`) stays
/// two plain dots, and `1.. =4` (the second dot not glued to `=`) stays
/// `..` + a plain `=`.
pub(crate) fn read_op(tokens: &[TokenTree], i: usize) -> Option<(Op, usize)> {
    let TokenTree::Punct(p) = tokens.get(i)? else { return None };
    let c = p.as_char();
    match c {
        '.' if p.spacing() == Spacing::Joint && is_punct_at(tokens, i + 1, '.') => {
            // `..` or `..=`: the inclusive form keeps its second dot Joint
            // (glued to `=`), matching `parse_range`'s historical check.
            if is_joint_punct_at(tokens, i + 1, '.') && is_punct_at(tokens, i + 2, '=') {
                Some((Op::DotDotEq, 3))
            } else {
                Some((Op::DotDot, 2))
            }
        }
        '-' if p.spacing() == Spacing::Joint && is_punct_at(tokens, i + 1, '>') => {
            Some((Op::Arrow, 2))
        }
        ':' if p.spacing() == Spacing::Joint && is_punct_at(tokens, i + 1, ':') => {
            Some((Op::ColonColon, 2))
        }
        _ => {
            let op = match c {
                '.' => Op::Dot,
                '-' => Op::Minus,
                ':' => Op::Colon,
                '=' => Op::Eq,
                ',' => Op::Comma,
                ';' => Op::Semicolon,
                '@' => Op::At,
                '!' => Op::Bang,
                '+' => Op::Plus,
                _ => return None,
            };
            Some((op, 1))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::TokenStream;

    fn ops(s: &str) -> Vec<(Op, usize)> {
        let v = s.parse::<TokenStream>().unwrap().into_iter().collect::<Vec<_>>();
        let mut out = vec![];
        let mut i = 0;
        while i < v.len() {
            match read_op(&v, i) {
                Some((op, len)) => {
                    out.push((op, len));
                    i += len;
                }
                None => i += 1,
            }
        }
        out
    }

    #[test]
    fn compound_shapes() {
        assert_eq!(
            ops("a .. b -> c :: d ..= e . f"),
            vec![
                (Op::DotDot, 2),
                (Op::Arrow, 2),
                (Op::ColonColon, 2),
                (Op::DotDotEq, 3),
                (Op::Dot, 1),
            ]
        );
    }

    #[test]
    fn spaced_dots_stay_single() {
        // `1 . . 4` — two spaced dots are two plain dots, not a range
        assert_eq!(ops("1 . . 4"), vec![(Op::Dot, 1), (Op::Dot, 1)]);
        // `1.. =4` — the second dot is not glued to `=`: `..` + plain `=`
        assert_eq!(ops("1 .. = 4"), vec![(Op::DotDot, 2), (Op::Eq, 1)]);
    }

    #[test]
    fn glued_dot_against_operand_is_plain() {
        // `self.0` — the `.` is Joint (glued to `0`) but not a range head
        assert_eq!(ops("self . 0"), vec![(Op::Dot, 1)]);
    }

    #[test]
    fn non_punct_skips() {
        assert!(ops("ident 3 (group)").is_empty());
    }
}
