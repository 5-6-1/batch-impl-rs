//! Angle group collection preprocessing.
//!
//! proc-macro2's tokenizer only groups `()`/`[]`/`{}`; `<>` stays as flat
//! Puncts. Before DSL parsing, this module pairs flat `<...>` into angle
//! groups (carried by `delimiter![<>]` = `Delimiter::None`), so downstream
//! parse layers no longer need `<>` depth tracking.
//!
//! Responsibilities and recursion rules: see [`angle_collect`];
//! [`render_angles`] is the output-side mirror.

use proc_macro2::{Group, TokenStream, TokenTree};

use crate::util::compile_error_str;
use crate::util::{bracket_is_passthrough, is_arrow};

/// Maximum recursion depth (aligned with v0.1's 128 levels).
///
/// Nested groups (`[[[...]]]`) and nested `<>` (`Vec<Vec<...>>`) are handled
/// recursively via [`angle_collect`]; deep nesting overflows the compiler stack
/// (measured STATUS_STACK_OVERFLOW at 30000 levels) — the entry counter
/// intercepts this, and valid DSL (group nesting ≤ 5) is completely unaffected.
pub(crate) const MAX_NEST_DEPTH: usize = 128;

/// Entry transformation: a single pass flattens None groups and pairs `<...>`.
///
/// - `Brace` groups (passthrough code) are not entered;
/// - `Paren` groups (DSL tuples) recurse; `Bracket` groups (DSL lists) recurse,
///   but `ident![...]` macro bodies / `#[...]` attributes are **not entered**
///   (their content may be arbitrary Rust, including comparison `<`);
/// - Flat `<`/`>` must be paired (the `>` of a `->` arrow does not
///   participate); an orphaned (unpaired) one errors — this is invalid input,
///   and once reported, downstream (scan/where/path scanning) no longer needs
///   `<>` depth tracking.
pub(crate) fn angle_collect(
    tokens: &[TokenTree],
) -> Result<Vec<TokenTree>, TokenStream> {
    angle_collect_at(tokens, 0)
}

fn angle_collect_at(
    tokens: &[TokenTree], depth: usize,
) -> Result<Vec<TokenTree>, TokenStream> {
    if depth > MAX_NEST_DEPTH {
        let sp = tokens
            .first()
            .map(|t| t.span())
            .unwrap_or_else(proc_macro2::Span::call_site);
        return Err(compile_error_str(
            &format!(
                "batch-impl: nesting depth exceeds {} levels (perhaps an accidental extra bracket)",
                MAX_NEST_DEPTH
            ),
            sp,
        ));
    }
    let mut out = vec![];
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            // Real None group (macro-variable output): content is DSL tokens, flatten
            TokenTree::Group(g) if g.delimiter() == delimiter![none] => {
                let inner: Vec<_> = g.stream().into_iter().collect();
                out.extend(angle_collect_at(&inner, depth + 1)?);
                i += 1;
            }
            // DSL tuple: recurse
            TokenTree::Group(g) if g.delimiter() == delimiter![()] => {
                let inner: Vec<_> = g.stream().into_iter().collect();
                let mut new_g = Group::new(
                    g.delimiter(),
                    angle_collect_at(&inner, depth + 1)?.into_iter().collect(),
                );
                new_g.set_span(g.span());
                out.push(new_g.into());
                i += 1;
            }
            // DSL list; `ident![...]` / `#[...]` passthrough (content is arbitrary Rust)
            TokenTree::Group(g) if g.delimiter() == delimiter![[]] => {
                if bracket_is_passthrough(tokens, i) {
                    out.push(tokens[i].clone());
                } else {
                    let inner: Vec<_> = g.stream().into_iter().collect();
                    let mut new_g = Group::new(
                        g.delimiter(),
                        angle_collect_at(&inner, depth + 1)?.into_iter().collect(),
                    );
                    new_g.set_span(g.span());
                    out.push(new_g.into());
                }
                i += 1;
            }
            // Passthrough code (body): do not enter
            TokenTree::Group(_) => {
                out.push(tokens[i].clone());
                i += 1;
            }
            // Flat `<`: pair to the matching `>` (the `>` of `->` does not
            // participate)
            TokenTree::Punct(p) if p.as_char() == '<' => {
                let Some(close) = find_angle_close(tokens, i) else {
                    return Err(compile_error_str(
                        "batch-impl: unclosed `<` (missing matching `>`)",
                        tokens[i].span(),
                    ));
                };
                let inner: Vec<_> = tokens[i + 1..close].to_vec();
                out.push(
                    Group::new(
                        delimiter![<>],
                        angle_collect_at(&inner, depth + 1)?.into_iter().collect(),
                    )
                    .into(),
                );
                i = close + 1;
            }
            // Extra `>` (not an arrow): invalid
            TokenTree::Punct(p) if p.as_char() == '>' && !is_arrow(tokens, i) => {
                return Err(compile_error_str(
                    "batch-impl: extra `>` (missing matching `<`)",
                    tokens[i].span(),
                ));
            }
            _ => {
                out.push(tokens[i].clone());
                i += 1;
            }
        }
    }
    Ok(out)
}

/// Find the matching `>` for `tokens[open]` (`<`): tracks nested `<` depth;
/// the `>` of a `->` arrow does not close. Returns the index of the matching
/// `>`; `None` if unclosed (the caller reports `unclosed <`).
fn find_angle_close(tokens: &[TokenTree], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (idx, token) in tokens.iter().enumerate().skip(open + 1) {
        if is_punct(token, '<') {
            depth += 1;
        } else if is_punct(token, '>') && !is_arrow(tokens, idx) {
            if depth == 0 {
                return Some(idx);
            }
            depth -= 1;
        }
    }
    None
}

fn is_punct(token: &TokenTree, ch: char) -> bool {
    matches!(token, TokenTree::Punct(p) if p.as_char() == ch)
}

/// Output transformation: recursively restores angle groups (`delimiter![<>]`)
/// to flat `<` + content + `>` tokens. Used to finalize the return values of
/// the three macro entries (quote interpolation would scatter angle groups
/// across the output).
///
/// Recursion rules match [`angle_collect`]: angle group → emit `<...>`
/// (recurse inside); `Paren`/`Bracket` (entered during pairing, may contain
/// nested angle groups) → rebuild and recurse; `Brace` (passthrough code that
/// `angle_collect` never entered → cannot contain angle groups) →
/// **passthrough as-is, no rebuild** (keeps spans, avoiding impact on
/// passthrough code and diagnostic mapping).
pub(crate) fn render_angles(stream: TokenStream) -> TokenStream {
    let mut out = TokenStream::new();
    for tt in stream {
        match tt {
            TokenTree::Group(g) if g.delimiter() == delimiter![<>] => {
                let inner = render_angles(g.stream());
                out.extend([TokenTree::from(proc_macro2::Punct::new(
                    '<',
                    proc_macro2::Spacing::Alone,
                ))]);
                out.extend(inner);
                out.extend([TokenTree::from(proc_macro2::Punct::new(
                    '>',
                    proc_macro2::Spacing::Alone,
                ))]);
            }
            TokenTree::Group(g)
                if matches!(g.delimiter(), delimiter![()] | delimiter![[]]) =>
            {
                let inner = render_angles(g.stream());
                // Rebuild and restore the original span (otherwise Bracket
                // groups such as doc attributes get call_site spans, affecting
                // span-based diagnostic mapping in clippy and others)
                let mut new_g = Group::new(g.delimiter(), inner);
                new_g.set_span(g.span());
                out.extend([TokenTree::Group(new_g)]);
            }
            // Brace (passthrough code): keep as-is — cannot contain angle
            // groups inside
            other => out.extend([other]),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::TokenStream as TS2;
    use std::str::FromStr;

    /// Roundtrip of entry collect + exit restore: `<...>` is paired into
    /// groups then restored to flat; the tokens are equivalent.
    fn roundtrip(s: &str) -> String {
        let ts: TS2 = FromStr::from_str(s).unwrap();
        let v: Vec<_> = ts.into_iter().collect();
        let collected = angle_collect(&v).unwrap();
        render_angles(collected.into_iter().collect()).to_string()
    }

    /// Recursion depth guard: 129 nested groups (> MAX_NEST_DEPTH) error
    /// instead of overflowing the stack.
    #[test]
    fn angle_nesting_limit() {
        let ts: TS2 = FromStr::from_str(&format!(
            "{}0{}",
            "[".repeat(MAX_NEST_DEPTH + 1),
            "]".repeat(MAX_NEST_DEPTH + 1)
        ))
        .unwrap();
        let v: Vec<_> = ts.into_iter().collect();
        let err = angle_collect(&v).unwrap_err().to_string();
        assert!(
            err.contains("nesting depth exceeds"),
            "expected depth-limit diagnostic, got: {err}"
        );
    }

    #[test]
    fn angle_roundtrip() {
        assert_eq!(roundtrip("Vec<T>"), "Vec < T >");
        assert_eq!(roundtrip("A<B<C>>"), "A < B < C > >");
        assert_eq!(
            roundtrip("Box<dyn Fn() + Send>"),
            "Box < dyn Fn () + Send >"
        );
        assert_eq!(roundtrip("<T: Clone> A<T>"), "< T : Clone > A < T >");
        assert_eq!(roundtrip("A<Item=T>"), "A < Item = T >");
        // the > of the -> arrow does not participate in pairing
        assert_eq!(roundtrip("fn(A) -> B"), "fn (A) -> B");
    }

    #[test]
    fn angle_unmatched_errors() {
        // Orphaned < / > is invalid input: reports compile_error! (no longer
        // passthrough)
        let ts: TS2 = FromStr::from_str("A <").unwrap();
        assert!(angle_collect(&ts.into_iter().collect::<Vec<_>>()).is_err());
        let ts: TS2 = FromStr::from_str("A >").unwrap();
        assert!(angle_collect(&ts.into_iter().collect::<Vec<_>>()).is_err());
        // `ident![...]` macro bodies are not entered: inner comparison < does
        // not error
        let ts: TS2 = FromStr::from_str("m![a < b]").unwrap();
        assert!(angle_collect(&ts.into_iter().collect::<Vec<_>>()).is_ok());
    }

    #[test]
    fn bracket_passthrough_guards() {
        // `ident![...]` macro bodies and `#[...]` attributes are not entered
        // (content is arbitrary Rust, incl. comparison <)
        for s in ["m![a < b]", "#[a < b]", "#[#zzz{1}]"] {
            let ts: TS2 = FromStr::from_str(s).unwrap();
            assert!(
                angle_collect(&ts.into_iter().collect::<Vec<_>>()).is_ok(),
                "input {s} should passthrough"
            );
        }
    }

    #[test]
    fn none_group_flattened() {
        // Real None group (macro-variable expansion output): after flattening,
        // the <...> in its content pairs as usual
        let inner: TS2 = FromStr::from_str("Vec<T>").unwrap();
        let none = proc_macro2::Group::new(delimiter![none], inner);
        let collected = angle_collect(&[none.into()]).unwrap();
        let rendered = render_angles(collected.into_iter().collect());
        assert_eq!(rendered.to_string(), "Vec < T >");
    }

    #[test]
    fn render_rebuilds_nested_groups() {
        // Paren/Bracket groups were entered during pairing, so rendering
        // rebuilds them and inner angle groups restore as usual; Brace
        // passthrough is not entered (the `<` in its body is not paired).
        // Note: span preservation cannot be unit-tested — in fallback mode
        // `Span::mixed_site()` is call_site, and `Span::eq` is gated by
        // procmacro2_semver_exempt.
        assert_eq!(
            roundtrip("[Vec<T>, (U, W<X>)]"),
            "[Vec < T > , (U , W < X >)]"
        );
        assert_eq!(roundtrip("{ a < b }"), "{ a < b }");
    }
}
