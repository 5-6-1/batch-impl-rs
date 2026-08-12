//! Shared utilities: token cursor scanning ([`Cursor`]) and compile-time diagnostic
//! construction ([`compile_error_str`]).
//!
//! This directory has no business dependencies and is referenced by every layer; `mod.rs`
//! aggregates the re-exports, so callers write `crate::util::X` (not submodule paths).

pub(crate) mod diagnostic;
pub(crate) mod scan;

pub(crate) use diagnostic::*;
pub(crate) use scan::*;

use proc_macro2::{TokenStream, TokenTree};

/// Maximum recursion depth (aligned with v0.1's 128 levels) for every
/// recursive token-tree walker (angle pairing / constant expansion / constant
/// value reference validation). Nested groups deep enough overflow the
/// compiler stack (measured STATUS_STACK_OVERFLOW at 30000 levels) — the
/// entry counter intercepts this, and valid DSL (group nesting ≤ 5) is
/// completely unaffected.
pub(crate) const MAX_NEST_DEPTH: usize = 128;

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
