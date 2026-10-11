//! Repeat-block expansion for impl bodies (shape template variadic segments):
//! `@( <pattern>, )..` repeats the pattern once per element of the variadic
//! segment(s) it references.
//!
//! A block's repetition count is the common length of its driving segments
//! (the `@ident` references inside; all must be equal-length, else error).
//! Each round `i` substitutes:
//! - `@ident` → that capture's value in the selected segment's i-th row,
//!   spliced directly without rewriting the captured target subtree, and
//! - `@N` → the numeric literal `N + i` (a plain index cursor — the caller
//!   writes the path prefix, e.g. `self.@1` for a segment starting at 1).
//!
//! Nested blocks finish independently before outer rounds copy their results
//! (Cartesian semantics); copied values are not substituted again. Source
//! tokens use the fixed-slot mapping once. Outside a block, fresh carriers
//! remain supported, but bare segment references and cursors are errors.

use proc_macro2::{Ident, Span, TokenStream, TokenTree};
use std::cell::Cell;

#[path = "repeat_output.rs"]
pub(crate) mod output;
use output::Output;

use super::repeat_drivers::fix_literal_at;
use crate::codegen::VarSeg;
use crate::util::{MAX_NEST_DEPTH, TokenDomain, compile_error_str, depth_err, is_punct_at};

/// The output-token budget of one body's repeat-block expansion. Nested
/// blocks multiply their round counts (Cartesian semantics — the output is
/// ∏len over the nesting levels), so the product itself needs a cap: depth
/// alone bounds recursion, not emission. 64k tokens is ~100× a generous
/// single impl body; real matrices sit orders of magnitude below it.
pub(crate) const MAX_REPEAT_TOKENS: usize = 1 << 16;

/// The per-expansion context, shared by every recursion level: the declared
/// segments (capture rows), the fixed-slot mapping, the fresh
/// context and the fresh-binding switch. One struct instead of parallel
/// parameter lists — a new concern joins as a field, not as another thread
/// through five signatures.
pub(crate) struct RepeatCtx<'a> {
    /// Explicit body switches authorize lexical rewriting inside every group.
    pub(crate) domain: TokenDomain,
    pub(crate) segs: &'a [VarSeg],
    /// Fixed-slot replacements for source tokens; row captures live in `segs`.
    pub(crate) map: &'a crate::codegen::Mapping,
    /// The fresh display names (`@{N}` references).
    pub(crate) fresh: &'a super::FreshCtx,
    /// The `impl{@0..}` switch driving cursor-only blocks.
    pub(crate) binding: Option<crate::ast::fresh_protocol::FreshRef>,
    /// Remaining final-output budget. Temporary nested expansions have local
    /// bounds; their tokens are not debited again at every ancestor level.
    pub(crate) budget: Cell<usize>,
}

/// Expands every repeat block in a body token stream.
/// Cursor-only blocks use the selected fresh-binding scope when no template
/// segment supplies their length.
pub(crate) fn expand_repeat_blocks(
    tokens: TokenStream, cx: &RepeatCtx,
) -> Result<TokenStream, TokenStream> {
    let v = tokens.into_iter().collect::<Vec<_>>();
    let out = expand_stream(&v, cx, 0, cx.budget.get())?;
    cx.budget.set(cx.budget.get().saturating_sub(out.count));
    Ok(out.tokens.into_iter().collect())
}

/// Expands repeat blocks and fresh carriers, applying fixed slots to source
/// tokens. Group traversal follows the caller's body-substitution domain.
fn expand_stream(
    tokens: &[TokenTree], cx: &RepeatCtx, depth: usize, limit: usize,
) -> Result<Output, TokenStream> {
    if depth > MAX_NEST_DEPTH {
        return Err(depth_err(tokens, ""));
    }
    let mut out = Output::new(limit);
    let mut i = 0;
    while let Some(cur) = tokens.get(i) {
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
                    out.append(
                        expand_block(
                            &body,
                            &sep,
                            cx,
                            depth + 1,
                            Some(id.clone()),
                            out.remaining(),
                        )?,
                        cur.span(),
                    )?;
                    i = next;
                }
                // `@ ( body ) [sep] ..` — the inter-round separator (`sep`)
                // is emitted between rounds, never after the last one (the
                // `$($A),*` vs `$($A,)*` distinction).
                _ if let Some((body, sep, next)) = parse_repeat_at(tokens, i + 1) => {
                    out.append(
                        expand_block(&body, &sep, cx, depth + 1, None, out.remaining())?,
                        cur.span(),
                    )?;
                    i = next;
                }
                // A fresh-ref carrier (`@{...}` — landed in the body by the
                // directive signature substitution of trait args) is **not**
                // a repeat block: resolve it now and mark its value generated.
                _ if let Some(g) = crate::ast::fresh_protocol::carrier_group_at(tokens, i) => {
                    out.extend(crate::codegen::expand_body_refs(
                        [cur.clone(), TokenTree::Group(g.clone())].into_iter().collect(),
                        cx.fresh,
                        cx.domain,
                    )?)?;
                    i += 2;
                }
                _ => {
                    return Err(compile_error_str(
                        "batch-impl: `@` inside an impl body must start a repeat block \
                         `@(...)..` (or `@ident(...)..` with the driving segment declared)",
                        cur.span(),
                    ));
                }
            }
            continue;
        }
        if let TokenTree::Group(g) = cur {
            if cx.domain.skips_group(tokens, i) {
                out.push(cur.clone())?;
                i += 1;
                continue;
            }
            if depth + 1 > MAX_NEST_DEPTH {
                return Err(depth_err(std::slice::from_ref(cur), ""));
            }
            let inner = g.stream().into_iter().collect::<Vec<_>>();
            let expanded =
                expand_stream(&inner, cx, depth + 1, out.resolved_group_limit(g, &inner, &[])?)?;
            out.resolved_group(g, &inner, &[], expanded)?;
            i += 1;
            continue;
        }
        if is_punct_at(tokens, i, '\'') {
            out.extend(crate::util::slice_window(tokens, i, 2).iter().cloned())?;
            i += 2;
            continue;
        }
        if let Some(repaired) = fix_literal_at(tokens, i) {
            out.extend(repaired)?;
        } else {
            super::repeat_drivers::emit_source(&mut out, cur, cx.map)?;
        }
        i += 1;
    }
    Ok(out)
}

/// Expands one repeat block: nested blocks first (their rounds are
/// independent and their completed output is copied), then `L` rounds of
/// source-marker substitution (`L` = the driving
/// segment's length — a declared `@ident` prefix, the block's inner segment
/// references, the template's unique segment for a cursor-only block, or the
/// fresh-binding switch's scope). Source `sep` tokens are resolved and emitted
/// **between** rounds (never after the last one) — the `$($A),*` form;
/// write the separator inside the body for the `$($A,)*` form.
fn expand_block(
    body: &[TokenTree], sep: &[TokenTree], cx: &RepeatCtx, depth: usize, driver: Option<Ident>,
    limit: usize,
) -> Result<Output, TokenStream> {
    if depth > MAX_NEST_DEPTH {
        return Err(depth_err(body, " in a repeat block"));
    }
    // Nested blocks finish independently. Provenance protects their values
    // from both outer cursors and fixed-slot substitutions.
    let body = expand_nested(body, cx, depth, MAX_REPEAT_TOKENS)?;
    let names = super::repeat_drivers::collect_drivers(&body.tokens, &body.origins, cx.domain)?;
    let span = body.tokens.first().map_or_else(Span::call_site, TokenTree::span);
    let selection = super::repeat_bindings::select(&names, cx.segs, driver.as_ref(), span)?;
    let len = match selection.len {
        Some(len) => len,
        None if cx.segs.len() == 1 => cx.segs.first().map_or(0, VarSeg::len),
        None if cx.segs.len() > 1 => {
            return Err(compile_error_str(
                "batch-impl: a cursor-only repeat block needs a driving \
                 segment — with several template segments write \
                 `@ident(...)..` declaring the driver",
                span,
            ));
        }
        None => binding_len(cx, &body.tokens)?,
    };
    // L rounds of marker substitution, with the source separator resolved
    // between rounds (never after the last one).
    let mut out = Output::new(limit);
    for round in 0..len {
        out.append(
            super::repeat_drivers::substitute(
                &body.tokens,
                &body.origins,
                cx,
                &selection,
                round,
                depth + 1,
                out.remaining(),
            )?,
            span,
        )?;
        if round + 1 < len {
            out.append(expand_stream(sep, cx, depth + 1, out.remaining())?, span)?;
        }
    }
    Ok(out)
}

/// Parses a repeat block's parts at `@ ( body ) [sep] ..`: returns the body
/// tokens, the source separator tokens between `)` and `..` (empty for the
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
    while let Some(cur) = tokens.get(j) {
        if is_punct_at(tokens, j, '.') && is_punct_at(tokens, j + 1, '.') {
            return Some((body, sep, j + 2));
        }
        sep.push(cur.clone());
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
    tokens: &[TokenTree], cx: &RepeatCtx, depth: usize, limit: usize,
) -> Result<Output, TokenStream> {
    if depth > MAX_NEST_DEPTH {
        return Err(depth_err(tokens, " in a repeat block"));
    }
    let mut out = Output::new(limit);
    let mut i = 0;
    while let Some(cur) = tokens.get(i) {
        // `@ident ( body ) [sep] ..` — declared driver
        if is_punct_at(tokens, i, '@')
            && let Some(TokenTree::Ident(id)) = tokens.get(i + 1)
            && let Some((body, sep, next)) = parse_repeat_at(tokens, i + 2)
        {
            out.append(
                expand_block(&body, &sep, cx, depth + 1, Some(id.clone()), out.remaining())?,
                cur.span(),
            )?;
            i = next;
            continue;
        }
        // `@ ( body ) [sep] ..`
        if is_punct_at(tokens, i, '@')
            && let Some((body, sep, next)) = parse_repeat_at(tokens, i + 1)
        {
            out.append(
                expand_block(&body, &sep, cx, depth + 1, None, out.remaining())?,
                cur.span(),
            )?;
            i = next;
            continue;
        }
        if let TokenTree::Group(g) = cur {
            if cx.domain.skips_group(tokens, i) {
                out.source(cur.clone());
                i += 1;
                continue;
            }
            if depth + 1 > MAX_NEST_DEPTH {
                return Err(depth_err(std::slice::from_ref(cur), ""));
            }
            let inner = g.stream().into_iter().collect::<Vec<_>>();
            let expanded = expand_nested(&inner, cx, depth + 1, out.remaining())?;
            out.template_group(g, expanded)?;
            i += 1;
            continue;
        }
        if let Some(repaired) = fix_literal_at(tokens, i) {
            for token in repaired {
                out.source(token);
            }
        } else {
            out.source(cur.clone());
        }
        i += 1;
    }
    Ok(out)
}
