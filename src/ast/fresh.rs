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
/// a `@` punct followed by a Brace group holding `inner`. Shared emitter of
/// every carrier protocol (fresh references, declarations, segment slots).
fn carrier_tokens(inner: String, span: proc_macro2::Span) -> TokenStream {
    let mut ts = TokenStream::new();
    let mut at = proc_macro2::Punct::new('@', proc_macro2::Spacing::Alone);
    at.set_span(span);
    ts.extend(std::iter::once(TokenTree::Punct(at)));
    // The spelled inner is always a valid token sequence; the default keeps
    // the no-panic promise under internal invariant drift.
    let parsed: TokenStream = inner.parse().unwrap_or_default();
    let mut g = proc_macro2::Group::new(proc_macro2::Delimiter::Brace, parsed);
    g.set_span(span);
    ts.extend(std::iter::once(TokenTree::Group(g)));
    ts
}

/// A **variadic-segment slot reference** — the structured identity of one
/// element of an `impl{...}` shape template's `ident@..` segment: the
/// user-written segment name plus the absolute leaf position it feeds.
/// Carries in token domains as `@{prefix_pos}` — the same self-delimiting
/// carrier shape as a fresh reference, so no minted identifier exists
/// between the repeat-block expansion and the slot-mapping rewrite.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SegRef {
    /// The user-written segment name (`A` in `impl{(A@..)}`).
    pub(crate) prefix: String,
    /// The absolute leaf tuple position this slot feeds (`start + round`).
    pub(crate) pos: usize,
}

impl SegRef {
    /// The carrier's inner spelling (`A_3`). The separator cannot occur in
    /// an identifier, so the split is unambiguous.
    #[allow(dead_code)]
    pub(crate) fn spell(&self) -> String {
        format!("{}_{}", self.prefix, self.pos)
    }

    /// Parses the inner spelling; `None` for anything else. The single
    /// authority for both directions of the encoding.
    pub(crate) fn parse(s: &str) -> Option<Self> {
        let (prefix, pos) = s.rsplit_once('_')?;
        (!prefix.is_empty()).then_some(Self { prefix: prefix.to_string(), pos: pos.parse().ok()? })
    }
}

/// Emits the `@{prefix_pos}` carrier of a segment-slot reference. Kept
/// alongside the parser: the two directions of the segment-slot encoding
/// (currently exercised by [`SegRef`] tests; the body-side emitter rides on
/// the documented positional names — see `repeat_drivers`).
#[allow(dead_code)]
pub(crate) fn seg_ref_tokens(r: &SegRef, span: proc_macro2::Span) -> TokenStream {
    carrier_tokens(r.spell(), span)
}

/// Parses a **declaration carrier**: a lone `@{g_i}` pair (single position,
/// grouped). Returns the structured identity; `None` for anything else —
/// user-written params, range carriers, malformed pairs.
pub(crate) fn decl_fresh_pos(tokens: &TokenStream) -> Option<(usize, usize)> {
    let v: Vec<_> = tokens.clone().into_iter().collect();
    match v.as_slice() {
        [TokenTree::Punct(p), TokenTree::Group(g)]
            if p.as_char() == '@' && g.delimiter() == proc_macro2::Delimiter::Brace =>
        {
            let inner: String =
                g.stream().into_iter().map(|t| t.to_string()).collect::<Vec<_>>().join("");
            match FreshRef::parse(&inner)? {
                FreshRef { group: Some(g), start: i, end: FreshEnd::Single } => Some((g, i)),
                _ => None,
            }
        }
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

/// Folds every **flat** position reference in `tokens` into the carrier form:
/// `@0` / `@g_i` / `@N..` / `@N..M` / `@N..=M` (and the deprecated
/// `@all_fresh`, normalized to `@{0..}`) become `@` + Brace groups. Existing
/// carriers pass through untouched, so this is idempotent — the single
/// normalization point for resolvers that may receive user-spelled input
/// (where predicates, blanket wrapper clauses). A malformed reference
/// (non-digit after `@`, malformed end, empty exclusive range) is left
/// as-is for the caller's validation to report.
pub(crate) fn fold_flat_refs(tokens: &[TokenTree]) -> Vec<TokenTree> {
    let mut out = Vec::with_capacity(tokens.len());
    let mut i = 0;
    while i < tokens.len() {
        let at_span = match &tokens[i] {
            TokenTree::Punct(p) if p.as_char() == '@' => p.span(),
            _ => {
                out.push(tokens[i].clone());
                i += 1;
                continue;
            }
        };
        // Already a carrier (`@{...}`): keep both tokens verbatim.
        if matches!(tokens.get(i + 1), Some(TokenTree::Group(g))
            if g.delimiter() == proc_macro2::Delimiter::Brace)
        {
            out.push(tokens[i].clone());
            out.push(tokens[i + 1].clone());
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
            let (group, start): (Option<usize>, usize) = if let Ok(n) = s.parse::<usize>() {
                (None, n)
            } else if let Some((l, n)) = s.split_once('_')
                && let (Ok(l), Ok(n)) = (l.parse::<usize>(), n.parse::<usize>())
            {
                (Some(l), n)
            } else {
                out.push(tokens[i].clone());
                i += 1;
                continue;
            };
            // Optional range tail: `..` (open) / `..=M` / `..M`.
            let mut consumed = 2usize;
            let end = if matches!(tokens.get(i + 2), Some(TokenTree::Punct(p)) if p.as_char() == '.')
                && matches!(tokens.get(i + 3), Some(TokenTree::Punct(p)) if p.as_char() == '.')
            {
                consumed = 4;
                let inclusive =
                    matches!(tokens.get(i + 4), Some(TokenTree::Punct(p)) if p.as_char() == '=');
                if inclusive {
                    consumed += 1;
                }
                match tokens.get(i + consumed) {
                    Some(TokenTree::Literal(el)) => match el.to_string().parse::<usize>() {
                        Ok(e) => {
                            consumed += 1;
                            if inclusive {
                                Some(FreshEnd::Closed(e))
                            } else if start < e {
                                Some(FreshEnd::Closed(e - 1))
                            } else {
                                // empty exclusive range — leave for validation
                                out.push(tokens[i].clone());
                                i += 1;
                                continue;
                            }
                        }
                        Err(_) => {
                            out.push(tokens[i].clone());
                            i += 1;
                            continue;
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
        out.push(tokens[i].clone());
        i += 1;
    }
    out
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
#[test]
fn probe_marker_final() {
    use quote::ToTokens;
    use syn::parse2;
    // The chosen marker and its near-miss shapes
    for s in ["[A;()]", "[A; ()]", "[(); A]", "[(); N]", "[A; 3]", "[A; N]", "[A; []]"] {
        let ts: proc_macro2::TokenStream = s.parse().unwrap();
        match parse2::<syn::Type>(ts) {
            Ok(t) => println!("{s:12} => OK: {}", t.to_token_stream()),
            Err(e) => println!("{s:12} => FAIL: {}", e),
        }
    }
    // Structure of the () length
    let ts: proc_macro2::TokenStream = "[A;()]".parse().unwrap();
    if let Ok(syn::Type::Array(a)) = parse2::<syn::Type>(ts) {
        println!("len tokens: {}", a.len.to_token_stream());
        match &a.len {
            syn::Expr::Tuple(tp) => println!("len = Expr::Tuple with {} elems", tp.elems.len()),
            o => println!("len kind: {:?}", std::mem::discriminant(o)),
        }
    }
}
