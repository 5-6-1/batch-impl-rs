//! Repeat-block expansion for impl bodies (shape template variadic segments):
//! `@( <pattern>, )..` repeats the pattern once per element of the variadic
//! segment(s) it references.
//!
//! A block's repetition count is the common length of its driving segments
//! (the `@ident` references inside; all must be equal-length, else error).
//! Each round `i` substitutes:
//! - `@ident` → the segment's i-th name (`prefix` + `start + i`, the leaf
//!   position-aligned numbering), and
//! - `@N` → the numeric literal `N + i` (a plain index cursor — the caller
//!   writes the path prefix, e.g. `self.@1` for a segment starting at 1).
//!
//! Nested blocks get independent rounds (Cartesian semantics: every outer
//! round re-runs the inner block over its own segment). The block body is
//! emitted verbatim each round (a trailing `,` separator stays a trailing
//! comma — legal in tuple/list contexts). Outside a block, `@` in a body is
//! an error: repeat blocks are the only legal `@` construct there.

use proc_macro2::{Group, Ident, Literal, Punct, Spacing, Span, TokenStream, TokenTree};

use crate::codegen::VarSeg;
use crate::util::{MAX_NEST_DEPTH, compile_error_str, depth_err, is_punct_at};

/// Expands every repeat block in a body token stream.
pub(crate) fn expand_repeat_blocks(
    tokens: TokenStream, segs: &[VarSeg],
) -> Result<TokenStream, TokenStream> {
    let v = fix_literal_at(tokens.into_iter().collect::<Vec<_>>());
    expand_stream(&v, segs, 0).map(|out| out.into_iter().collect())
}

/// Repairs the float-literal tokenization of `数字.@`: the tokenizer reads
/// `self.0.@0` as `self . 0. @ 0` (the `0.` becomes a float literal), which
/// would render `self.0.0` as two adjacent literals. Splitting the trailing
/// `.` off keeps the natural `self.0.@0` spelling working (the cursor then
/// expands into `self.0.0`, `self.0.1`, ...).
fn fix_literal_at(tokens: Vec<TokenTree>) -> Vec<TokenTree> {
    let mut out = vec![];
    let mut i = 0;
    while i < tokens.len() {
        if let TokenTree::Literal(lit) = &tokens[i] {
            let s = lit.to_string();
            if s.ends_with('.')
                && is_punct_at(&tokens, i + 1, '@')
                && let Ok(n) = s[..s.len() - 1].parse::<u64>()
            {
                out.push(TokenTree::Literal(Literal::u64_unsuffixed(n)));
                out.push(TokenTree::Punct(Punct::new('.', Spacing::Alone)));
                i += 1;
                continue;
            }
        }
        if let TokenTree::Group(g) = &tokens[i] {
            let inner = fix_literal_at(g.stream().into_iter().collect::<Vec<_>>());
            let mut ng = Group::new(g.delimiter(), inner.into_iter().collect());
            ng.set_span(g.span());
            out.push(TokenTree::Group(ng));
            i += 1;
            continue;
        }
        out.push(tokens[i].clone());
        i += 1;
    }
    out
}

/// Stream-level scan: expands `@( ... )..` blocks and recurses into groups;
/// any other `@` in a body is an error.
fn expand_stream(
    tokens: &[TokenTree], segs: &[VarSeg], depth: usize,
) -> Result<Vec<TokenTree>, TokenStream> {
    if depth > MAX_NEST_DEPTH {
        return Err(depth_err(tokens, ""));
    }
    let mut out = vec![];
    let mut i = 0;
    while i < tokens.len() {
        if is_punct_at(tokens, i, '@') {
            // `@ident ( body ) ..` — the driving segment is declared up
            // front (the length source; the body may use only `@N` cursors).
            if let Some(TokenTree::Ident(id)) = tokens.get(i + 1)
                && let Some(TokenTree::Group(g)) = tokens.get(i + 2)
                && g.delimiter() == delimiter![()]
                && is_punct_at(tokens, i + 3, '.')
                && is_punct_at(tokens, i + 4, '.')
            {
                let body = g.stream().into_iter().collect::<Vec<_>>();
                out.extend(expand_block(&body, segs, depth + 1, Some(id.clone()))?);
                i += 5;
                continue;
            }
            // `@ ( body ) ..`
            if let Some(TokenTree::Group(g)) = tokens.get(i + 1)
                && g.delimiter() == delimiter![()]
                && is_punct_at(tokens, i + 2, '.')
                && is_punct_at(tokens, i + 3, '.')
            {
                let body = g.stream().into_iter().collect::<Vec<_>>();
                out.extend(expand_block(&body, segs, depth + 1, None)?);
                i += 4;
                continue;
            }
            return Err(compile_error_str(
                "batch-impl: `@` inside an impl body must start a repeat block \
                 `@(...)..` (or `@ident(...)..` with the driving segment declared)",
                tokens[i].span(),
            ));
        }
        if let TokenTree::Group(g) = &tokens[i] {
            if depth + 1 > MAX_NEST_DEPTH {
                return Err(depth_err(&tokens[i..i + 1], ""));
            }
            let inner = g.stream().into_iter().collect::<Vec<_>>();
            let expanded = expand_stream(&inner, segs, depth + 1)?;
            let mut ng = Group::new(g.delimiter(), expanded.into_iter().collect());
            ng.set_span(g.span());
            out.push(TokenTree::Group(ng));
            i += 1;
            continue;
        }
        out.push(tokens[i].clone());
        i += 1;
    }
    Ok(out)
}

/// Expands one repeat block: nested blocks first (their rounds are
/// independent), then `L` rounds of marker substitution (`L` = the driving
/// segment's length — a declared `@ident` prefix, the block's inner segment
/// references, or the template's unique segment for a cursor-only block).
fn expand_block(
    body: &[TokenTree], segs: &[VarSeg], depth: usize, driver: Option<Ident>,
) -> Result<Vec<TokenTree>, TokenStream> {
    if depth > MAX_NEST_DEPTH {
        return Err(depth_err(body, " in a repeat block"));
    }
    // 1. Nested repeat blocks expand first (own rounds).
    let body = expand_nested(body, segs, depth)?;
    // 2. The inner segment references (prefixes + their common length).
    let (inner_prefixes, inner_len) = super::repeat_drivers::collect_drivers(&body, segs)?;
    // 3. The repetition count.
    let len = match driver {
        // Declared driver: it is the length source; any inner references
        // must point at the same segment.
        Some(id) => {
            let prefix = id.to_string();
            let Some(seg) = segs.iter().find(|s| s.prefix == prefix) else {
                return Err(compile_error_str(
                    &format!(
                        "batch-impl: repeat block driver `@{}` is not a variadic \
                         segment (the `impl{{...}}` template declares no `{}@..`)",
                        prefix, prefix,
                    ),
                    id.span(),
                ));
            };
            for p in &inner_prefixes {
                if *p != prefix {
                    return Err(compile_error_str(
                        &format!(
                            "batch-impl: repeat block driver `@{}` conflicts with the \
                             inner segment reference `@{}` (they must be the same)",
                            prefix, p,
                        ),
                        id.span(),
                    ));
                }
            }
            seg.len
        }
        // No declared driver: the inner references decide; a cursor-only
        // block binds the template's unique segment (its length is the only
        // possible one — no guessing).
        None => match inner_len {
            Some(l) => l,
            None if segs.len() == 1 => segs[0].len,
            None => {
                return Err(compile_error_str(
                    "batch-impl: a repeat block needs a driving segment to determine \
                     its length — write `@ident(...)..` with the segment declared, or \
                     reference a segment inside",
                    body.first().map_or_else(Span::call_site, |t| t.span()),
                ));
            }
        },
    };
    // 4. L rounds of marker substitution.
    let mut out = vec![];
    for round in 0..len {
        out.extend(super::repeat_drivers::substitute(&body, segs, round, depth + 1)?);
    }
    Ok(out)
}

/// Expands nested repeat blocks inside a block body, keeping `@ident` / `@N`
/// markers untouched (they are substituted per round by the outer block).
fn expand_nested(
    tokens: &[TokenTree], segs: &[VarSeg], depth: usize,
) -> Result<Vec<TokenTree>, TokenStream> {
    if depth > MAX_NEST_DEPTH {
        return Err(depth_err(tokens, " in a repeat block"));
    }
    let mut out = vec![];
    let mut i = 0;
    while i < tokens.len() {
        // `@ident ( body ) ..` — declared driver
        if is_punct_at(tokens, i, '@')
            && let Some(TokenTree::Ident(id)) = tokens.get(i + 1)
            && let Some(TokenTree::Group(g)) = tokens.get(i + 2)
            && g.delimiter() == delimiter![()]
            && is_punct_at(tokens, i + 3, '.')
            && is_punct_at(tokens, i + 4, '.')
        {
            let body = g.stream().into_iter().collect::<Vec<_>>();
            out.extend(expand_block(&body, segs, depth + 1, Some(id.clone()))?);
            i += 5;
            continue;
        }
        // `@ ( body ) ..`
        if is_punct_at(tokens, i, '@')
            && let Some(TokenTree::Group(g)) = tokens.get(i + 1)
            && g.delimiter() == delimiter![()]
            && is_punct_at(tokens, i + 2, '.')
            && is_punct_at(tokens, i + 3, '.')
        {
            let body = g.stream().into_iter().collect::<Vec<_>>();
            out.extend(expand_block(&body, segs, depth + 1, None)?);
            i += 4;
            continue;
        }
        if let TokenTree::Group(g) = &tokens[i] {
            if depth + 1 > MAX_NEST_DEPTH {
                return Err(depth_err(&tokens[i..i + 1], ""));
            }
            let inner = g.stream().into_iter().collect::<Vec<_>>();
            let expanded = expand_nested(&inner, segs, depth + 1)?;
            let mut ng = Group::new(g.delimiter(), expanded.into_iter().collect());
            ng.set_span(g.span());
            out.push(TokenTree::Group(ng));
            i += 1;
            continue;
        }
        out.push(tokens[i].clone());
        i += 1;
    }
    Ok(out)
}
