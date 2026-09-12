//! The fresh-generic protocol — the single source of truth for both sides of
//! the macro-meta layer:
//!
//! - **Declarations** ([`fresh_decl_tokens`] / [`decl_fresh_pos`]) — the
//!   apply layer mints a declaration carrier for each generator position;
//!   its identity is the structured `(group, position)` pair, never a name
//!   string. The codegen stage assigns display names (`P0, P1, ...`) once
//!   per impl, collision-aware against everything the impl already uses.
//! - **References** ([`FreshRef`] / [`FreshEnd`]) — `@N` / `@g_i` / range
//!   references ride the Ty tree structurally (`TyKind::Fresh`) and carry in
//!   token domains as the self-delimiting `@{...}` group; [`fold_flat_refs`]
//!   normalizes user-spelled input to that carrier, and
//!   [`FreshRef::parse`] / [`FreshRef::spell`] are the two directions of the
//!   encoding so parser and emitter can never drift.
//!
//! Declarations and references share one token shape — the self-delimiting
//! `@{...}` carrier — so no reserved identifier pattern exists anywhere in
//! the pipeline and nothing internal can collide with user code or leak
//! into rendered output.

use proc_macro2::{TokenStream, TokenTree};

use crate::util::{Op, compile_error_str, read_op, tokens_to_string};

/// Mints the **declaration carrier** of generator fresh `(group g, position
/// i)`: the same self-delimiting `@{g_i}` form a reference carries. The
/// declaration's identity is this structured pair — dedup across cloned
/// generators compares parsed pairs, not spellings, so token spacing can
/// never split one logical declaration in two.
pub(crate) fn fresh_decl_tokens(g: usize, i: usize) -> TokenStream {
    fresh_ref_tokens(
        FreshRef { group: Some(g), start: i, end: FreshEnd::Single },
        proc_macro2::Span::call_site(),
    )
}

/// Emits the self-delimiting carrier for an arbitrary spelled reference —
/// a `@` punct followed by a Brace group holding `inner`. The shared emitter
/// of the fresh-reference carrier protocol (declarations + references).
fn carrier_tokens(inner: String, span: proc_macro2::Span) -> TokenStream {
    let mut ts = TokenStream::new();
    let mut at = proc_macro2::Punct::new('@', proc_macro2::Spacing::Alone);
    at.set_span(span);
    ts.extend(std::iter::once(TokenTree::Punct(at)));
    // The spelled inner is always a valid token sequence; the default keeps
    // the no-panic promise under internal invariant drift.
    let parsed = inner.parse().unwrap_or_default();
    let mut g = proc_macro2::Group::new(proc_macro2::Delimiter::Brace, parsed);
    g.set_span(span);
    ts.extend(std::iter::once(TokenTree::Group(g)));
    ts
}

/// Parses a **declaration carrier**: a lone `@{g_i}` pair (single position,
/// grouped). Returns the structured identity; `None` for anything else —
/// user-written params, range carriers, malformed pairs.
pub(crate) fn decl_fresh_pos(tokens: &TokenStream) -> Option<(usize, usize)> {
    let v = tokens.clone().into_iter().collect::<Vec<_>>();
    let inner = carrier_inner_at(&v, 0)?;
    match FreshRef::parse(&inner)? {
        FreshRef { group: Some(g), start: i, end: FreshEnd::Single } => Some((g, i)),
        _ => None,
    }
}

/// A resolved `@N` / `@g_i` / `@N..` / `@N..M` position reference — the
/// structured carrier that rides in the [`Ty`](crate::ast::Ty) tree
/// (`TyKind::Fresh`) and renders to the self-delimiting token form
/// `@{...}` (`@{0}`, `@{1_0..}`, `@{0..=3}`) for the token-level resolvers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FreshRef {
    /// `Some(L)` for the grouped forms (`@g_i` / `@L_N..` — within generator
    /// group L, stable across array dispatch); `None` is the flat form.
    pub(crate) group: Option<usize>,
    /// Flattened index or in-group position (numbered from 0).
    pub(crate) start: usize,
    pub(crate) end: FreshEnd,
}

/// The extent of a [`FreshRef`]: a single position (`@N` / `@g_i`), an open
/// range to the last fresh (`@N..` / `@L_N..` — empty when `start` is past
/// the end), or a closed range (`@N..M` / `@N..=M` normalized to inclusive).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FreshEnd {
    Single,
    Open,
    Closed(usize),
}

impl FreshRef {
    /// Whether this reference re-opens into several names (a range form).
    pub(crate) fn is_range(&self) -> bool {
        !matches!(self.end, FreshEnd::Single)
    }

    /// The `@{...}` inner spelling (`0`, `1_0..`, `0..=3`) — shared by the
    /// token emitter and the parser so the two can never drift.
    pub(crate) fn spell(&self) -> String {
        let head = match self.group {
            Some(l) => format!("{l}_{}", self.start),
            None => format!("{}", self.start),
        };
        match self.end {
            FreshEnd::Single => head,
            FreshEnd::Open => format!("{head}.."),
            FreshEnd::Closed(e) => format!("{head}..={e}"),
        }
    }

    /// Parses the inner spelling of an `@{...}` group; `None` for anything
    /// else. The single authority for both directions of the carrier.
    pub(crate) fn parse(s: &str) -> Option<Self> {
        let (group, rest) = match s.split_once('_') {
            // A grouped head needs a following position part; a plain number
            // has none (`split_once` on `0..=3` would misread `0..=3` — check
            // the tail parses as digits before accepting the split).
            Some((l, tail)) if tail.split(['.', '_']).next()?.parse::<usize>().is_ok() => {
                (Some(l.parse::<usize>().ok()?), tail)
            }
            _ => (None, s),
        };
        if let Some((start, end)) = rest.split_once("..=") {
            let start = start.parse::<usize>().ok()?;
            let end = end.parse::<usize>().ok()?;
            (start <= end).then_some(FreshRef { group, start, end: FreshEnd::Closed(end) })
        } else if let Some(stripped) = rest.strip_suffix("..") {
            let start = stripped.parse::<usize>().ok()?;
            (!stripped.is_empty()).then_some(FreshRef { group, start, end: FreshEnd::Open })
        } else {
            Some(FreshRef { group, start: rest.parse::<usize>().ok()?, end: FreshEnd::Single })
        }
    }
}

/// Emits the self-delimiting carrier tokens of a reference — a `@` punct
/// followed by a Brace group holding [`FreshRef::spell`]. The group is an
/// atomic unit for every token walker, so the reference survives any pass
/// untouched and can only be consumed by the resolvers that match this shape.
pub(crate) fn fresh_ref_tokens(r: FreshRef, span: proc_macro2::Span) -> TokenStream {
    carrier_tokens(r.spell(), span)
}

/// One **input error of an `@` position reference**, encoded once and rendered
/// into whichever channel the reporting site sits in.
///
/// The same handful of mistakes are diagnosed from four places — the
/// type-position block parser, the token-domain folder, the `resolve_at_refs`
/// rewrite and the `@{...}` resolvers — and before this type each site spelled
/// its own message: the four families had drifted into 3 / 3 / 4 / 2 copies, one
/// copy had lost its `format!` arguments (the user saw a literal `@{}..{}`), and
/// the type-position sites wrapped an **item-form** `compile_error!(…);` into a
/// *type*, so rustc reported `expected one of ',' or '>'` and the macro's own
/// message never appeared (second review round, F1/F2/F4).
///
/// One construction site, two renderings: [`Self::into_ty`] for a type position
/// (a real error node, so the entry's error channel aggregates it and no
/// half-built impl is emitted) and [`Self::into_stream`] for callers that return
/// `Err` to the entry. The spellings below are deliberate: each has been the
/// user-visible wording of its situation since 0.9.x, and the UI snapshots pin
/// them — this type converges them without rewriting them.
pub(crate) struct AtRefError {
    message: String,
    span: proc_macro2::Span,
}

impl AtRefError {
    /// `@` with no position digit after it (`@` at the end of a type).
    pub(crate) fn position_digit(span: proc_macro2::Span) -> Self {
        Self::new(
            "batch-impl: `@` in a type must be followed by a position digit \
             (e.g. `@0` or `@0_1`)",
            span,
        )
    }

    /// A token follows the `@` but it is not a position digit (`@foo`, `@((u8))`).
    pub(crate) fn not_a_position_digit(span: proc_macro2::Span) -> Self {
        Self::new("batch-impl: `@` in a type must be a position digit (e.g. `@0` or `@0_1`)", span)
    }

    /// `@N..M` whose end is not a number (`@0..x`).
    pub(crate) fn range_end_not_a_number(span: proc_macro2::Span) -> Self {
        Self::new("batch-impl: a `@N..M` range must end with a number (e.g. `@0..=2`)", span)
    }

    /// `@N..M` with `N >= M` — the inclusive protocol cannot represent it, and
    /// the message **names the numbers** (the pre-convergence copy printed a
    /// literal `@{}..{}`).
    pub(crate) fn empty_exclusive_range(start: usize, end: usize, span: proc_macro2::Span) -> Self {
        Self::new(
            &format!("batch-impl: empty exclusive range `@{start}..{end}` (start not below end)"),
            span,
        )
    }

    /// A `@{...}` carrier whose content is not a position reference
    /// (`@{foo}`, `@{}`).
    pub(crate) fn position_reference(span: proc_macro2::Span) -> Self {
        Self::new(
            "batch-impl: `@{...}` must hold a position reference \
             (e.g. `@{0}`, `@{1_0..}`, `@{0..=3}`)",
            span,
        )
    }

    fn new(message: &str, span: proc_macro2::Span) -> Self {
        Self { message: message.to_string(), span }
    }

    /// The type-position rendering: an error node (`TyKind::Error`), which the
    /// entry's error aggregator collects and emits on the error channel.
    pub(crate) fn into_ty(self) -> crate::ast::Ty {
        crate::apply::err_ty_at(&self.message, self.span)
    }

    /// The item-form rendering (`::core::compile_error!(…);`) for `Err` returns.
    pub(crate) fn into_stream(self) -> TokenStream {
        compile_error_str(&self.message, self.span)
    }
}

/// Whether `tokens[i]` opens a **carrier** (`@` punct + Brace group) — the
/// atomic token shape shared by declarations and references. Every walker
/// that must not touch carriers passes them through behind this single
/// test; the shape is owned here so the protocol and its recognition can
/// never drift apart.
pub(crate) fn is_carrier_at(tokens: &[TokenTree], i: usize) -> bool {
    carrier_group_at(tokens, i).is_some()
}

/// The carrier's Brace group at `tokens[i]` (`@` + Brace), when present.
/// The single recognition + extraction — callers that need the group (or
/// its content) use this instead of `is_carrier_at` plus a manual
/// `tokens[i + 1]` re-destructure (which can drift from the recognition
/// test and needs `unreachable!`).
pub(crate) fn carrier_group_at(tokens: &[TokenTree], i: usize) -> Option<&proc_macro2::Group> {
    let g = match tokens.get(i + 1) {
        Some(TokenTree::Group(g)) if g.delimiter() == proc_macro2::Delimiter::Brace => g,
        _ => return None,
    };
    matches!(tokens.get(i), Some(TokenTree::Punct(p)) if p.as_char() == '@').then_some(g)
}

/// The `@{...}` carrier's inner content as a string (`@{0_0}` → `"0_0"`),
/// when `tokens[i]` opens a carrier. **One** test + extraction — callers
/// that need the content use this instead of `is_carrier_at` plus a manual
/// `tokens[i + 1]` re-destructure.
pub(crate) fn carrier_inner_at(tokens: &[TokenTree], i: usize) -> Option<String> {
    carrier_group_at(tokens, i).map(carrier_inner)
}

/// The `@{...}` carrier's inner content as a string (`@{0_0}` → `"0_0"`).
/// The single extraction every carrier reader uses — the token-to-string
/// join must not be re-derived across the codebase.
pub(crate) fn carrier_inner(g: &proc_macro2::Group) -> String {
    tokens_to_string(&g.stream().into_iter().collect::<Vec<_>>())
}

/// Whether a body token stream contains a **user-level fresh-position carrier**
/// (`@{...}`) that the body-slot switch (`impl{@{}}`) must gate. A **grouped**
/// carrier (`@{0_0}`) is macro-generated (blanket delegates mint `@{g_i}` for
/// their fresh generic) and exempt: it always expands to a fresh name. A flat
/// `@{N}` in an expression, return type, etc. is user-written and requires
/// the switch ("declare what you use"). Returns the carrier's span when one
/// is found, so the caller can point the diagnostic at the offending token.
pub(crate) fn body_has_carrier(tokens: &TokenStream) -> Option<proc_macro2::Span> {
    let v = tokens.clone().into_iter().collect::<Vec<_>>();
    carrier_at_any(&v, 0)
}

fn carrier_at_any(tokens: &[TokenTree], depth: usize) -> Option<proc_macro2::Span> {
    if depth > crate::util::MAX_NEST_DEPTH {
        return None;
    }
    let mut i = 0;
    while let Some(cur) = tokens.get(i) {
        if is_carrier_at(tokens, i) && !is_macro_generated_carrier(tokens, i) {
            return Some(cur.span());
        }
        if let TokenTree::Group(g) = cur {
            let inner = g.stream().into_iter().collect::<Vec<_>>();
            if let Some(sp) = carrier_at_any(&inner, depth + 1) {
                return Some(sp);
            }
        }
        i += 1;
    }
    None
}

/// Whether the carrier at `i` (`@{...}`) is **macro-generated**: its Brace
/// group holds a grouped reference (`@{0_0}` — a `g_i` position, the shape
/// blanket delegates mint for their fresh generic) or a range (`@{0..}` —
/// the shape trait-argument substitution produces ranges). Flat single
/// `@{0}` references are user-written.
fn is_macro_generated_carrier(tokens: &[TokenTree], i: usize) -> bool {
    let Some(inner) = carrier_inner_at(tokens, i) else { return false };
    // A grouped head (`0_0`) contains an underscore; a range contains `..`.
    // A flat single reference (`0`) has neither.
    inner.contains('_') || inner.contains("..")
}

/// Folds every **flat** position reference in `tokens` into the carrier form:
/// `@0` / `@g_i` / `@N..` / `@N..M` / `@N..=M` (and the deprecated
/// `@all_fresh`, normalized to `@{0..}`) become `@` + Brace groups. Existing
/// carriers pass through untouched, so this is idempotent — the single
/// normalization point for resolvers that may receive user-spelled input
/// (where predicates, blanket wrapper clauses). A malformed reference reports
/// the same targeted diagnostics as the type-position path (a non-position
/// token after `@`, a non-numeric range end, an empty exclusive range) —
/// the where side must not leak a raw `@` into the rendered clause where the
/// type side errors.
pub(crate) fn fold_flat_refs(tokens: &[TokenTree]) -> Result<Vec<TokenTree>, TokenStream> {
    let mut out = Vec::with_capacity(tokens.len());
    let mut i = 0;
    while let Some(cur) = tokens.get(i) {
        let at_span = match cur {
            TokenTree::Punct(p) if p.as_char() == '@' => p.span(),
            _ => {
                out.push(cur.clone());
                i += 1;
                continue;
            }
        };
        // Already a carrier (`@{...}`): keep both tokens verbatim.
        if let Some(g) = carrier_group_at(tokens, i) {
            out.push(cur.clone());
            out.push(TokenTree::Group(g.clone()));
            i += 2;
            continue;
        }
        // Deprecated batch form: `@all_fresh` ≡ `@{0..}`.
        if let Some(TokenTree::Ident(id)) = tokens.get(i + 1)
            && id == "all_fresh"
        {
            out.extend(fresh_ref_tokens(
                FreshRef { group: None, start: 0, end: FreshEnd::Open },
                at_span,
            ));
            i += 2;
            continue;
        }
        if let Some(TokenTree::Literal(lit)) = tokens.get(i + 1) {
            let s = lit.to_string();
            // Head classification: `N` (flat) or `L_N` (grouped).
            let (group, start) = if let Ok(n) = s.parse::<usize>() {
                (None, n)
            } else if let Some((l, n)) = s.split_once('_')
                && let (Ok(l), Ok(n)) = (l.parse::<usize>(), n.parse::<usize>())
            {
                (Some(l), n)
            } else {
                return Err(AtRefError::position_digit(at_span).into_stream());
            };
            // Optional range tail: `..` (open) / `..=M` / `..M`.
            let mut consumed = 2usize;
            let end = if let Some((op, _)) = read_op(tokens, i + 2)
                && matches!(op, Op::DotDot | Op::DotDotEq)
            {
                let inclusive = matches!(op, Op::DotDotEq);
                // The guard above admits only `..` / `..=`; the width is read
                // without an `unreachable!` — a panic inside a proc macro is a
                // compiler ICE.
                consumed = 2 + if inclusive { 3 } else { 2 };
                match tokens.get(i + consumed) {
                    Some(TokenTree::Literal(el)) => match el.to_string().parse::<usize>() {
                        Ok(e) => {
                            consumed += 1;
                            if inclusive {
                                Some(FreshEnd::Closed(e))
                            } else if start < e {
                                Some(FreshEnd::Closed(e - 1))
                            } else {
                                // empty exclusive range — the same diagnostic
                                // the type-position path reports (F1: the
                                // numbers are formatted in, not left as `{}`)
                                return Err(AtRefError::empty_exclusive_range(start, e, at_span)
                                    .into_stream());
                            }
                        }
                        Err(_) => {
                            return Err(AtRefError::range_end_not_a_number(at_span).into_stream());
                        }
                    },
                    _ => Some(FreshEnd::Open),
                }
            } else {
                Some(FreshEnd::Single)
            };
            if let Some(end) = end {
                out.extend(fresh_ref_tokens(FreshRef { group, start, end }, at_span));
                i += consumed;
                continue;
            }
        }
        // `@` followed by anything else (a non-`all_fresh` ident, a punct, a
        // non-Brace group, or nothing) — a malformed reference, reported like
        // the type-position path instead of leaking the raw `@` through.
        return Err(AtRefError::position_digit(at_span).into_stream());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::FreshEnd;

    #[test]
    fn fresh_ref_spell_parse_roundtrip() {
        for r in [
            FreshRef { group: None, start: 0, end: FreshEnd::Single },
            FreshRef { group: None, start: 1, end: FreshEnd::Open },
            FreshRef { group: None, start: 0, end: FreshEnd::Closed(2) },
            FreshRef { group: Some(0), start: 0, end: FreshEnd::Single },
            FreshRef { group: Some(1), start: 0, end: FreshEnd::Open },
            FreshRef { group: Some(1), start: 1, end: FreshEnd::Closed(3) },
        ] {
            assert_eq!(FreshRef::parse(&r.spell()), Some(r), "{}", r.spell());
        }
    }

    #[test]
    fn fresh_ref_invalid_forms() {
        for s in ["", "x", "0..x", "1_", "2..1", "0_1_2"] {
            assert_eq!(FreshRef::parse(s), None, "{s}");
        }
    }

    #[test]
    fn decl_carriers_roundtrip() {
        // The declaration carrier mints and parses back to the same identity.
        assert_eq!(decl_fresh_pos(&fresh_decl_tokens(0, 1)), Some((0, 1)));
        assert_eq!(decl_fresh_pos(&fresh_decl_tokens(3, 12)), Some((3, 12)));
        // Ranges and flat refs are not declarations.
        let range = fresh_ref_tokens(
            FreshRef { group: None, start: 0, end: FreshEnd::Open },
            proc_macro2::Span::call_site(),
        );
        assert_eq!(decl_fresh_pos(&range), None);
        assert_eq!(decl_fresh_pos(&"T".parse::<TokenStream>().unwrap()), None);
    }
}
