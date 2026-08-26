//! Repeat-block expansion for impl bodies (shape template variadic segments):
//! `@( <pattern>, )..` repeats the pattern once per element of the variadic
//! segment(s) it references.
//!
//! A block's repetition count is the common length of its driving segments
//! (the `@ident` references inside; all must be equal-length, else error).
//! Each round `i` substitutes:
//! - `@ident` → the segment's i-th **bound element**, spliced directly from
//!   the shape mapping (the `$(...)*` semantics — the round's output shows
//!   the actual leaf subtree, no intermediate spelling), and
//! - `@N` → the numeric literal `N + i` (a plain index cursor — the caller
//!   writes the path prefix, e.g. `self.@1` for a segment starting at 1).
//!
//! Nested blocks get independent rounds (Cartesian semantics: every outer
//! round re-runs the inner block over its own segment). The block body is
//! emitted verbatim each round (a trailing `,` separator stays a trailing
//! comma — legal in tuple/list contexts). Outside a block, `@` in a body is
//! an error: repeat blocks are the only legal `@` construct there.

use proc_macro2::{Group, Ident, Span, TokenStream, TokenTree};
use std::cell::Cell;

use super::repeat_drivers::fix_literal_at;
use crate::codegen::VarSeg;
use crate::util::{MAX_NEST_DEPTH, compile_error_str, depth_err, is_punct_at};

/// The output-token budget of one body's repeat-block expansion. Nested
/// blocks multiply their round counts (Cartesian semantics — the output is
/// ∏len over the nesting levels), so the product itself needs a cap: depth
/// alone bounds recursion, not emission. 64k tokens is ~100× a generous
/// single impl body; real matrices sit orders of magnitude below it.
pub(crate) const MAX_REPEAT_TOKENS: usize = 1 << 16;

/// The per-expansion context, shared by every recursion level: the declared
/// segments (structure), the shape mapping (bound values), the fresh
/// context and the fresh-binding switch. One struct instead of parallel
/// parameter lists — a new concern joins as a field, not as another thread
/// through five signatures.
pub(crate) struct RepeatCtx<'a> {
    pub(crate) segs: &'a [VarSeg],
    /// The value source of `@ident` substitution (the `(prefix, position)`
    /// bindings produced by the shape match).
    pub(crate) map: &'a crate::codegen::Mapping,
    /// The fresh display names (`@{N}` references).
    pub(crate) fresh: &'a super::FreshCtx,
    /// The `impl{@0..}` switch driving cursor-only blocks.
    pub(crate) binding: Option<crate::ast::fresh::FreshRef>,
    /// Remaining output budget ([`MAX_REPEAT_TOKENS`]), spent by every block
    /// as it assembles its rounds. Interior-mutable: the expansion walks an
    /// immutable context.
    pub(crate) budget: Cell<usize>,
}

/// Expands every repeat block in a body token stream.
///
/// A cursor-only block with no template variadic segment to drive it
/// (`@(args.@0,)..` in a spec whose template has no `ident@..` segment)
/// repeats once per fresh — the bound-generator arity (`Fn()0..N`) becomes
/// the body's repetition count.
pub(crate) fn expand_repeat_blocks(
    tokens: TokenStream, cx: &RepeatCtx,
) -> Result<TokenStream, TokenStream> {
    let v = fix_literal_at(tokens.into_iter().collect::<Vec<_>>());
    expand_stream(&v, cx, 0).map(|out| out.into_iter().collect())
}

/// Stream-level scan: expands `@( ... )..` blocks and recurses into groups;
/// any other `@` in a body is an error.
fn expand_stream(
    tokens: &[TokenTree], cx: &RepeatCtx, depth: usize,
) -> Result<Vec<TokenTree>, TokenStream> {
    if depth > MAX_NEST_DEPTH {
        return Err(depth_err(tokens, ""));
    }
    let mut out = vec![];
    let mut i = 0;
    while i < tokens.len() {
        if is_punct_at(tokens, i, '@') {
            // The `@` dispatch, as match-arm if-let guards (1.95): each arm
            // pattern-matches its own precondition, failing over to the next
            // arm instead of nesting `if let`s.
            match tokens.get(i + 1) {
                // `@ident ( body ) [sep] ..` — the driving segment declared
                // up front (the length source; the body may use only `@N`
                // cursors)
                Some(TokenTree::Ident(id))
                    if let Some((body, sep, next)) = parse_repeat_at(tokens, i + 2) =>
                {
                    out.extend(expand_block(&body, &sep, cx, depth + 1, Some(id.clone()))?);
                    i = next;
                }
                // `@ ( body ) [sep] ..` — the inter-round separator (`sep`)
                // is emitted between rounds, never after the last one (the
                // `$($A),*` vs `$($A,)*` distinction).
                _ if let Some((body, sep, next)) = parse_repeat_at(tokens, i + 1) => {
                    out.extend(expand_block(&body, &sep, cx, depth + 1, None)?);
                    i = next;
                }
                // A fresh-ref carrier (`@{...}` — landed in the body by the
                // directive signature substitution of trait args) is **not**
                // a repeat block: pass it through for the later range
                // re-opening pass (`expand_range_refs` in `generate_parts`).
                _ if crate::ast::fresh::is_carrier_at(tokens, i) => {
                    out.push(tokens[i].clone());
                    out.push(tokens[i + 1].clone());
                    i += 2;
                }
                _ => {
                    return Err(compile_error_str(
                        "batch-impl: `@` inside an impl body must start a repeat block \
                         `@(...)..` (or `@ident(...)..` with the driving segment declared)",
                        tokens[i].span(),
                    ));
                }
            }
            continue;
        }
        if let TokenTree::Group(g) = &tokens[i] {
            if depth + 1 > MAX_NEST_DEPTH {
                return Err(depth_err(&tokens[i..i + 1], ""));
            }
            let inner = g.stream().into_iter().collect::<Vec<_>>();
            let expanded = expand_stream(&inner, cx, depth + 1)?;
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
/// references, the template's unique segment for a cursor-only block, or the
/// fresh-binding switch's scope). The literal `sep` tokens are emitted
/// **between** rounds (never after the last one) — the `$($A),*` form;
/// write the separator inside the body for the `$($A,)*` form.
fn expand_block(
    body: &[TokenTree], sep: &[TokenTree], cx: &RepeatCtx, depth: usize, driver: Option<Ident>,
) -> Result<Vec<TokenTree>, TokenStream> {
    if depth > MAX_NEST_DEPTH {
        return Err(depth_err(body, " in a repeat block"));
    }
    // 1. Nested repeat blocks expand first (own rounds).
    let body = expand_nested(body, cx, depth)?;
    // 2. The inner segment references (prefixes + their common length).
    let (inner_prefixes, inner_len) = super::repeat_drivers::collect_drivers(&body, cx.segs)?;
    // 3. The repetition count.
    let len = match driver {
        // Declared driver: it is the length source; any inner references
        // must point at the same segment.
        Some(id) => {
            let prefix = id.to_string();
            let Some(seg) = cx.segs.iter().find(|s| s.prefix == prefix) else {
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
        // possible one — no guessing) or the fresh-binding switch's scope
        // when there is no segment at all (the bound-generator arity drives
        // the body — zero bound fresh = zero rounds, the arity-0 impl).
        // Several segments without a reference cannot pick a length —
        // reject.
        None => match inner_len {
            Some(l) => l,
            None if cx.segs.len() == 1 => cx.segs[0].len,
            None if cx.segs.len() > 1 => {
                return Err(compile_error_str(
                    "batch-impl: a cursor-only repeat block needs a driving \
                     segment — with several template segments write \
                     `@ident(...)..` declaring the driver",
                    body.first().map_or_else(Span::call_site, |t| t.span()),
                ));
            }
            None => binding_len(cx, &body)?,
        },
    };
    // 4. L rounds of marker substitution, with the literal separator
    //    between rounds (never after the last one).
    let mut out = vec![];
    for round in 0..len {
        out.extend(super::repeat_drivers::substitute(&body, cx, round, depth + 1)?);
        if round + 1 < len {
            out.extend(sep.iter().cloned());
        }
    }
    // Spend the output budget: nested rounds already sit inside `out` (the
    // inner blocks expanded in step 1), so this single deduction covers the
    // whole ∏len of every nesting level below.
    let exceeded = out.len() > cx.budget.get();
    cx.budget.update(|b| b.saturating_sub(out.len()));
    if exceeded {
        return Err(compile_error_str(
            &format!(
                "batch-impl: repeat-block expansion produces {} tokens (limit {}); \
                 reduce the nesting depth or segment sizes",
                out.len(),
                MAX_REPEAT_TOKENS,
            ),
            body.first().map_or_else(Span::call_site, |t| t.span()),
        ));
    }
    Ok(out)
}

/// Parses a repeat block's parts at `@ ( body ) [sep] ..`: returns the body
/// tokens, the literal separator tokens between `)` and `..` (empty for the
/// plain `@(...)..` form), and the index just past `..`. `None` when the
/// tokens are not a repeat block (missing group or missing `..`).
fn parse_repeat_at(
    tokens: &[TokenTree], at: usize,
) -> Option<(Vec<TokenTree>, Vec<TokenTree>, usize)> {
    let TokenTree::Group(g) = tokens.get(at)? else {
        return None;
    };
    if g.delimiter() != delimiter![()] {
        return None;
    }
    let body = g.stream().into_iter().collect::<Vec<_>>();
    let mut j = at + 1;
    let mut sep = vec![];
    while j < tokens.len() {
        if is_punct_at(tokens, j, '.') && is_punct_at(tokens, j + 1, '.') {
            return Some((body, sep, j + 2));
        }
        sep.push(tokens[j].clone());
        j += 1;
    }
    None
}

/// The fresh-binding switch's scope length: how many fresh generics the
/// binding range covers (all fresh for a flat `@N..`, one group for
/// `@L_N..`, a closed run for `@N..=M`). `None` (no switch) errors — the
/// fresh-driven body modification is off without `impl{@0..}`.
fn binding_len(cx: &RepeatCtx, body: &[TokenTree]) -> Result<usize, TokenStream> {
    let Some(range) = cx.binding else {
        return Err(compile_error_str(
            "batch-impl: a repeat block needs a driving segment or a fresh-binding \
             switch (`impl{@0..}`) to determine its length",
            body.first().map_or_else(Span::call_site, |t| t.span()),
        ));
    };
    let scope_len = match range.group {
        Some(g) => cx.fresh.names.iter().filter(|&&(gg, _, _)| gg == g).count(),
        None => cx.fresh.names.len(),
    };
    crate::codegen::range_refs::range_count(&range, scope_len, Span::call_site())
}

/// Expands nested repeat blocks inside a block body, keeping `@ident` / `@N`
/// markers untouched (they are substituted per round by the outer block).
fn expand_nested(
    tokens: &[TokenTree], cx: &RepeatCtx, depth: usize,
) -> Result<Vec<TokenTree>, TokenStream> {
    if depth > MAX_NEST_DEPTH {
        return Err(depth_err(tokens, " in a repeat block"));
    }
    let mut out = vec![];
    let mut i = 0;
    while i < tokens.len() {
        // `@ident ( body ) [sep] ..` — declared driver
        if is_punct_at(tokens, i, '@')
            && let Some(TokenTree::Ident(id)) = tokens.get(i + 1)
            && let Some((body, sep, next)) = parse_repeat_at(tokens, i + 2)
        {
            out.extend(expand_block(&body, &sep, cx, depth + 1, Some(id.clone()))?);
            i = next;
            continue;
        }
        // `@ ( body ) [sep] ..`
        if is_punct_at(tokens, i, '@')
            && let Some((body, sep, next)) = parse_repeat_at(tokens, i + 1)
        {
            out.extend(expand_block(&body, &sep, cx, depth + 1, None)?);
            i = next;
            continue;
        }
        if let TokenTree::Group(g) = &tokens[i] {
            if depth + 1 > MAX_NEST_DEPTH {
                return Err(depth_err(&tokens[i..i + 1], ""));
            }
            let inner = g.stream().into_iter().collect::<Vec<_>>();
            let expanded = expand_nested(&inner, cx, depth + 1)?;
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
