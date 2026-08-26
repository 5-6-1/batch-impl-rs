//! Shared utilities: token cursor scanning ([`Cursor`]) and compile-time diagnostic
//! construction ([`compile_error_str`]).
//!
//! This directory has no business dependencies and is referenced by every layer; `mod.rs`
//! aggregates the re-exports, so callers write `crate::util::X` (not submodule paths).

pub(crate) mod diagnostic;
pub(crate) mod punct_ops;
pub(crate) mod scan;
pub(crate) mod subst;

pub(crate) use diagnostic::*;
pub(crate) use punct_ops::*;
pub(crate) use scan::*;

use proc_macro2::{Span, TokenStream, TokenTree};

/// Maximum recursion depth (aligned with v0.1's 128 levels) for every
/// recursive token-tree walker (angle pairing / constant expansion / constant
/// value reference validation). Nested groups deep enough overflow the
/// compiler stack (measured STATUS_STACK_OVERFLOW at 30000 levels) — the
/// entry counter intercepts this, and valid DSL (group nesting ≤ 5) is
/// completely unaffected.
pub(crate) const MAX_NEST_DEPTH: usize = 128;

/// Whether `a`'s span ends exactly where `b`'s begins (same line) — the
/// token-level **adjacency** test that [`proc_macro2::Spacing`] cannot
/// provide for range dots: the second dot of `..` lexes as `Alone` even when
/// glued to the following ident (`@u8..u128`), so "glued" must be read off
/// the byte positions. Requires the `span-locations` feature (enabled).
/// Macro-synthesized tokens carry call-site spans whose positions compare
/// arbitrarily — callers treat a `false` as "not adjacent" only when a
/// real source position is expected.
pub(crate) fn spans_adjacent(a: Span, b: Span) -> bool {
    let (ea, sb) = (a.end(), b.start());
    ea.line == sb.line && ea.column == sb.column
}

/// Builds the standard nesting-depth diagnostic (span from the first token).
/// `what` extends the message when the recursion happens inside a specific
/// structure (e.g. `" in a constant value"`) — one construction site so the
/// three recursive walkers cannot drift apart.
pub(crate) fn depth_err(tokens: &[TokenTree], what: &str) -> TokenStream {
    let sp = tokens.first().map_or_else(proc_macro2::Span::call_site, |t| t.span());
    diagnostic::compile_error_str(
        &format!(
            "batch-impl: nesting depth exceeds {} levels{} (perhaps an accidental extra bracket)",
            MAX_NEST_DEPTH, what
        ),
        sp,
    )
}

/// N-way Cartesian product over per-dimension candidate lists. The single
/// authority for Cartesian expansion — shared by the apply layer's power
/// (`pow_cartesian`, one list repeated `n` times) and the AST layer's
/// array-argument distribution (a candidate list per tuple/generic slot).
/// Each combination picks one element from every dimension, in document
/// order.
///
/// The would-be product size is checked **before each allocation** and the
/// growth is capped at `limit`: a huge user list (`(T1..Tk).N` with k×N large)
/// would otherwise exhaust memory or overflow the capacity multiplication
/// (a debug-build panic) before any caller-side check could run. `Err`
/// carries the would-be size so callers can render the same over-limit
/// diagnostic they already use.
pub(crate) fn cartesian<T: Clone>(dims: &[Vec<T>], limit: usize) -> Result<Vec<Vec<T>>, usize> {
    let mut combos: Vec<Vec<T>> = vec![vec![]];
    for candidates in dims {
        let next_len = combos.len().saturating_mul(candidates.len());
        if next_len > limit {
            return Err(next_len);
        }
        let mut next = Vec::with_capacity(next_len);
        for existing in &combos {
            for c in candidates {
                let mut combo = existing.clone();
                combo.push(c.clone());
                next.push(combo);
            }
        }
        combos = next;
    }
    Ok(combos)
}
