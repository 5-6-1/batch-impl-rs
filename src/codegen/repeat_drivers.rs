//! Repeat-block driver collection and marker substitution: the two helpers
//! of the repeat-block expansion (`expand_repeat_blocks` in `repeat.rs`).
//! `collect_drivers` finds source capture references for `repeat_bindings`
//! to resolve; `substitute` emits one selected row and advances numeric
//! cursors. Completed nested output and inserted values stay unchanged.

use super::repeat::output::{Origin, Output, generated, group_origins};
use super::repeat_bindings::Selection;
use proc_macro2::{Literal, Punct, Spacing, TokenStream, TokenTree};

/// Repairs the float-literal tokenization of `数字.@`: the tokenizer reads
/// `self.0.@0` as `self . 0. @ 0` (the `0.` becomes a float literal), which
/// would render `self.0.0` as two adjacent literals. Splitting the trailing
/// `.` off keeps the natural `self.0.@0` spelling working (the cursor then
/// expands into `self.0.0`, `self.0.1`, ...).
pub(super) fn fix_literal_at(tokens: &[TokenTree], at: usize) -> Option<[TokenTree; 2]> {
    let TokenTree::Literal(lit) = tokens.get(at)? else { return None };
    if !is_punct_at(tokens, at + 1, '@') {
        return None;
    }
    let (field, dot) = crate::util::split_tuple_field_dot(lit)?;
    Some([TokenTree::Literal(field), TokenTree::Punct(dot)])
}

use crate::util::{
    MAX_NEST_DEPTH, Op, compile_error_str, depth_err, is_punct_at, read_op, tokens_to_string,
};

/// `self.@0.1` lexes its two indices as one literal. Split only an
/// unsuffixed decimal pair in a field position; floats elsewhere stay Rust.
fn cursor_field(
    tokens: &[TokenTree], at: usize, literal: &Literal,
) -> Option<(usize, [TokenTree; 2])> {
    let previous = at.checked_sub(1)?;
    if !matches!(read_op(tokens, previous), Some((Op::Dot, 1)))
        || previous.checked_sub(1).is_some_and(|before| {
            matches!(read_op(tokens, before), Some((Op::DotDot | Op::DotDotEq, _)))
        })
    {
        return None;
    }
    let spelling = literal.to_string();
    let (cursor, field) = spelling.split_once('.')?;
    if cursor.is_empty()
        || field.is_empty()
        || !cursor.bytes().all(|b| b.is_ascii_digit())
        || !field.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let cursor = cursor.parse::<usize>().ok()?;
    let mut field = field.parse::<Literal>().ok()?;
    field.set_span(literal.span());
    let mut dot = Punct::new('.', Spacing::Alone);
    dot.set_span(literal.span());
    Some((cursor, [TokenTree::Punct(dot), TokenTree::Literal(field)]))
}

/// Capture names referenced by unfinished source, in first-appearance order.
/// Already-generated nested values contribute neither names nor drivers.
pub(crate) fn collect_drivers(
    tokens: &[TokenTree], origins: &[Origin], domain: crate::util::TokenDomain,
) -> Result<Vec<String>, TokenStream> {
    let mut prefixes = vec![];
    let mut i = 0;
    while let Some(cur) = tokens.get(i) {
        if generated(origins, i) {
            i += 1;
            continue;
        }
        if is_punct_at(tokens, i, '@') {
            if matches!(tokens.get(i + 1), Some(TokenTree::Literal(_)))
                || matches!(tokens.get(i + 1), Some(TokenTree::Group(g)) if g.delimiter() == delimiter![{}])
            {
                i += 2;
                continue;
            }
            let Some(TokenTree::Ident(id)) = tokens.get(i + 1) else {
                return Err(compile_error_str(
                    "batch-impl: `@` inside a repeat block must be followed by a \
                     segment name (`@ident`) or an index (`@N`)",
                    cur.span(),
                ));
            };
            let prefix = id.to_string();
            if !prefixes.contains(&prefix) {
                prefixes.push(prefix);
            }
            i += 2;
            continue;
        }
        if let TokenTree::Group(g) = cur
            && !domain.skips_group(tokens, i)
        {
            let inner = g.stream().into_iter().collect::<Vec<_>>();
            for p in collect_drivers(&inner, group_origins(origins, i), domain)? {
                if !prefixes.contains(&p) {
                    prefixes.push(p);
                }
            }
        }
        i += 1;
    }
    Ok(prefixes)
}

/// Substitutes source markers of one round: `@ident` → the selected segment
/// row's capture value (spliced directly), `@N` → `N + i`, `@{N}` → the N-th fresh
/// generic's name (the fresh-binding switch's name reference — fixed, not
/// per-round), and `@{@N}` → the `(N + i)`-th fresh's name (the per-round
/// form — each round names its own fresh).
pub(crate) fn substitute(
    tokens: &[TokenTree], origins: &[Origin], cx: &super::RepeatCtx, selection: &Selection,
    round: usize, depth: usize, limit: usize,
) -> Result<Output, TokenStream> {
    if depth > MAX_NEST_DEPTH {
        return Err(depth_err(tokens, ""));
    }
    let mut out = Output::new(limit);
    let mut i = 0;
    while let Some(cur) = tokens.get(i) {
        if generated(origins, i) {
            out.push(cur.clone())?;
            i += 1;
            continue;
        }
        // A fresh-ref carrier (`@{g_i}`) inside a repeat block: `@{N}` is a
        // **fixed fresh-name reference** (the successor of the retired `@@N`
        // spelling — the same `@{...}` shape the body uses, one `@` consumed),
        // resolved here against the fresh context. `@{@N}` is its **cursor**
        // form — the index is `N + round`, so each round names its own fresh
        // (`(@(@{@N}::foo()),..)` on three freshs →
        // `(P0::foo(), P1::foo(), P2::foo())`). A **range** carrier
        // (`@{0..}`) or grouped carrier resolves here through the shared
        // fresh-reference resolver. Its result is never new repeat source.
        if let Some(g) = crate::ast::fresh_protocol::carrier_group_at(tokens, i) {
            let inner_tokens = g.stream().into_iter().collect::<Vec<_>>();
            let index = if is_punct_at(&inner_tokens, 0, '@') {
                match inner_tokens.as_slice() {
                    [TokenTree::Punct(_), TokenTree::Literal(lit)] => {
                        match lit.to_string().parse::<usize>() {
                            Ok(n) => n.saturating_add(round),
                            Err(_) => {
                                return Err(compile_error_str(
                                    "batch-impl: `@{@...}` must be followed by an index \
                                     (`@{@0}`) — the per-round fresh reference",
                                    cur.span(),
                                ));
                            }
                        }
                    }
                    _ => {
                        return Err(compile_error_str(
                            "batch-impl: `@{@...}` must be followed by an index \
                             (`@{@0}`) — the per-round fresh reference",
                            cur.span(),
                        ));
                    }
                }
            } else {
                let inner = tokens_to_string(&inner_tokens);
                match inner.parse::<usize>() {
                    Ok(n) => n,
                    // Resolve a range or grouped carrier without scanning
                    // any previously inserted target tokens.
                    Err(_) => {
                        out.extend(crate::codegen::expand_body_refs(
                            [cur.clone(), TokenTree::Group(g.clone())].into_iter().collect(),
                            cx.fresh,
                            cx.domain,
                        )?)?;
                        i += 2;
                        continue;
                    }
                }
            };
            let Some((_, _, name)) = cx.fresh.names.get(index) else {
                return Err(compile_error_str(
                    &format!(
                        "batch-impl: `@{{{}}}` is out of range — this impl has {} \
                         fresh generics (numbered from 0 in document order)",
                        index,
                        cx.fresh.names.len(),
                    ),
                    cur.span(),
                ));
            };
            out.extend(name.clone())?;
            i += 2;
            continue;
        }
        if is_punct_at(tokens, i, '@') {
            match tokens.get(i + 1) {
                Some(TokenTree::Ident(id)) => {
                    let prefix = id.to_string();
                    let Some(seg) = selection.segment(&prefix, cx.segs) else {
                        // Verified by collect_drivers; defensive (no-panic).
                        return Err(compile_error_str(
                            &format!("batch-impl: unknown variadic segment `@{}`", prefix),
                            id.span(),
                        ));
                    };
                    // All columns of a selected segment use this same row.
                    // The target position is used only for diagnostics;
                    // inserted values never become source for another pass.
                    let pos = seg.start.saturating_add(round);
                    let Some(value) = seg.rows.get(round).and_then(|row| row.capture(&prefix))
                    else {
                        return Err(compile_error_str(
                            &format!(
                                "batch-impl: internal error — variadic segment `{}@..` \
                                 has no binding for element position {}",
                                prefix, pos,
                            ),
                            id.span(),
                        ));
                    };
                    out.extend(value.clone())?;
                    i += 2;
                    continue;
                }
                Some(TokenTree::Literal(lit)) => {
                    let parsed =
                        lit.to_string().parse::<usize>().ok().map(|n| (n, None)).or_else(|| {
                            cursor_field(tokens, i, lit).map(|(n, tail)| (n, Some(tail)))
                        });
                    let Some((n, tail)) = parsed else {
                        return Err(compile_error_str(
                            "batch-impl: a repeat cursor requires an unsuffixed decimal integer \
                             (`@0`); nested tuple fields can be written as `self.@0.1` or \
                             `(self.@0).1`",
                            lit.span(),
                        ));
                    };
                    // saturating_add: a `@18446744073709551615`-style cursor
                    // literal can parse to usize::MAX; a raw `n + round`
                    // would overflow in debug builds (no-panic promise).
                    let val = Literal::u64_unsuffixed((n.saturating_add(round)) as u64);
                    out.push(TokenTree::Literal(val))?;
                    if let Some(tail) = tail {
                        out.extend(tail)?;
                    }
                    i += 2;
                    continue;
                }
                _ => {
                    return Err(compile_error_str(
                        "batch-impl: `@` inside a repeat block must be followed by a \
                         segment name (`@ident`) or an index (`@N`)",
                        cur.span(),
                    ));
                }
            }
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
            let substituted = substitute(
                &inner,
                group_origins(origins, i),
                cx,
                selection,
                round,
                depth + 1,
                out.resolved_group_limit(g, &inner, group_origins(origins, i))?,
            )?;
            out.resolved_group(g, &inner, group_origins(origins, i), substituted)?;
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
            emit_source(&mut out, cur, cx.map)?;
        }
        i += 1;
    }
    Ok(out)
}

/// Apply fixed slots to source tokens exactly once. Bound target tokens are
/// emitted directly, so neither an outer slot nor a later round can rewrite them.
pub(super) fn emit_source(
    out: &mut Output, token: &TokenTree, mapping: &crate::codegen::Mapping,
) -> Result<(), TokenStream> {
    if let TokenTree::Ident(id) = token
        && let Some((_, value)) = mapping.slots().iter().find(|(name, _)| id == name)
    {
        out.extend(value.clone())
    } else {
        out.push(token.clone())
    }
}
