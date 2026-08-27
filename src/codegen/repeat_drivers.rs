//! Repeat-block driver collection and marker substitution: the two helpers
//! of the repeat-block expansion (`expand_repeat_blocks` in `repeat.rs`).
//! `collect_drivers` finds the `@ident` segment references of a block body
//! and their common length; `substitute` rewrites the markers of one round
//! (`@ident` → the segment's i-th bound element, spliced directly — the
//! `$(...)*` semantics; `@N` → `N + i`).

use proc_macro2::{Group, Literal, Punct, Spacing, TokenStream, TokenTree};

use crate::codegen::VarSeg;
/// Repairs the float-literal tokenization of `数字.@`: the tokenizer reads
/// `self.0.@0` as `self . 0. @ 0` (the `0.` becomes a float literal), which
/// would render `self.0.0` as two adjacent literals. Splitting the trailing
/// `.` off keeps the natural `self.0.@0` spelling working (the cursor then
/// expands into `self.0.0`, `self.0.1`, ...).
pub(super) fn fix_literal_at(tokens: Vec<TokenTree>) -> Vec<TokenTree> {
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

use crate::util::{MAX_NEST_DEPTH, compile_error_str, depth_err, is_punct_at, tokens_to_string};

/// The inner segment references of a block body: the deduplicated `@ident`
/// prefixes (first-appearance order) and their common length (`None` when the
/// block references no segment — a cursor-only block).
pub(crate) fn collect_drivers(
    tokens: &[TokenTree], segs: &[VarSeg],
) -> Result<(Vec<String>, Option<usize>), TokenStream> {
    let mut prefixes = vec![];
    let mut len = None;
    let mut i = 0;
    while i < tokens.len() {
        if is_punct_at(tokens, i, '@') {
            // `@N` index cursors are not segment references either.
            if matches!(tokens.get(i + 1), Some(TokenTree::Literal(_))) {
                i += 2;
                continue;
            }
            // A fresh-ref carrier from an already-expanded nested round
            // (`@{g_i}`, emitted by the directive signature substitution):
            // not a driver — pass through.
            if matches!(tokens.get(i + 1), Some(TokenTree::Group(g))
                if g.delimiter() == delimiter![{}])
            {
                i += 2;
                continue;
            }
            let Some(TokenTree::Ident(id)) = tokens.get(i + 1) else {
                return Err(compile_error_str(
                    "batch-impl: `@` inside a repeat block must be followed by a \
                     segment name (`@ident`) or an index (`@N`)",
                    tokens[i].span(),
                ));
            };
            let prefix = id.to_string();
            let Some(seg) = segs.iter().find(|s| s.prefix == prefix) else {
                return Err(compile_error_str(
                    &format!(
                        "batch-impl: repeat block references unknown variadic segment \
                         `@{}` (the `impl{{...}}` template declares no `{}@..`)",
                        prefix, prefix,
                    ),
                    id.span(),
                ));
            };
            if !prefixes.contains(&prefix) {
                prefixes.push(prefix);
            }
            match len {
                None => len = Some(seg.len),
                Some(l) if l != seg.len => {
                    return Err(compile_error_str(
                        &format!(
                            "batch-impl: repeat block segments have different lengths \
                             ({} vs {}); all referenced segments must be equal-length",
                            l, seg.len,
                        ),
                        id.span(),
                    ));
                }
                _ => {}
            }
            i += 2;
            continue;
        }
        if let TokenTree::Group(g) = &tokens[i] {
            let inner = g.stream().into_iter().collect::<Vec<_>>();
            let (p, l) = collect_drivers(&inner, segs)?;
            for p in p {
                if !prefixes.contains(&p) {
                    prefixes.push(p);
                }
            }
            match (len, l) {
                (None, _) => len = l,
                (Some(a), Some(b)) if a != b => {
                    return Err(compile_error_str(
                        "batch-impl: repeat block segments have different lengths; all \
                         referenced segments must be equal-length",
                        tokens[i].span(),
                    ));
                }
                _ => {}
            }
            i += 1;
            continue;
        }
        i += 1;
    }
    Ok((prefixes, len))
}

/// Substitutes the markers of one round: `@ident` → the segment's i-th
/// **bound element** (spliced directly from the shape mapping — the
/// `$(...)*` semantics; no intermediate name or carrier exists between the
/// expansion and the output), `@N` → `N + i`, `@{N}` → the N-th fresh
/// generic's name (the fresh-binding switch's name reference — fixed, not
/// per-round), and `@{@N}` → the `(N + i)`-th fresh's name (the per-round
/// form — each round names its own fresh).
pub(crate) fn substitute(
    tokens: &[TokenTree], cx: &super::RepeatCtx, round: usize, depth: usize,
) -> Result<Vec<TokenTree>, TokenStream> {
    if depth > MAX_NEST_DEPTH {
        return Err(depth_err(tokens, ""));
    }
    let mut out = vec![];
    let mut i = 0;
    while i < tokens.len() {
        // A fresh-ref carrier (`@{g_i}`) inside a repeat block: `@{N}` is a
        // **fixed fresh-name reference** (the successor of the retired `@@N`
        // spelling — the same `@{...}` shape the body uses, one `@` consumed),
        // resolved here against the fresh context. `@{@N}` is its **cursor**
        // form — the index is `N + round`, so each round names its own fresh
        // (`(@(@{@N}::foo()),..)` on three freshs →
        // `(P0::foo(), P1::foo(), P2::foo())`). A **range** carrier
        // (`@{0..}`) passes through untouched for the later range re-opening
        // pass (`expand_range_refs`).
        if crate::ast::fresh::is_carrier_at(tokens, i) {
            let Some(TokenTree::Group(g)) = tokens.get(i + 1) else {
                unreachable!("matched above");
            };
            let inner_tokens = g.stream().into_iter().collect::<Vec<_>>();
            let index = if is_punct_at(&inner_tokens, 0, '@') {
                match inner_tokens.as_slice() {
                    [TokenTree::Punct(_), TokenTree::Literal(lit)] => {
                        match lit.to_string().parse::<usize>() {
                            Ok(n) => n + round,
                            Err(_) => {
                                return Err(compile_error_str(
                                    "batch-impl: `@{@...}` must be followed by an index \
                                     (`@{@0}`) — the per-round fresh reference",
                                    tokens[i].span(),
                                ));
                            }
                        }
                    }
                    _ => {
                        return Err(compile_error_str(
                            "batch-impl: `@{@...}` must be followed by an index \
                             (`@{@0}`) — the per-round fresh reference",
                            tokens[i].span(),
                        ));
                    }
                }
            } else {
                let inner = tokens_to_string(&inner_tokens);
                match inner.parse::<usize>() {
                    Ok(n) => n,
                    // A range or grouped carrier: pass through for range
                    // re-opening.
                    Err(_) => {
                        out.push(tokens[i].clone());
                        out.push(tokens[i + 1].clone());
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
                    tokens[i].span(),
                ));
            };
            out.extend(name.clone());
            i += 2;
            continue;
        }
        if is_punct_at(tokens, i, '@') {
            match tokens.get(i + 1) {
                Some(TokenTree::Ident(id)) => {
                    let prefix = id.to_string();
                    let Some(seg) = cx.segs.iter().find(|s| s.prefix == prefix) else {
                        // Verified by collect_drivers; defensive (no-panic).
                        return Err(compile_error_str(
                            &format!("batch-impl: unknown variadic segment `@{}`", prefix),
                            id.span(),
                        ));
                    };
                    // Splice the slot's **bound element** directly — the
                    // `$(...)*` semantics: the round's output shows the
                    // actual leaf subtree, no intermediate spelling exists.
                    let pos = seg.start + round;
                    let Some(value) = cx.map.seg_value(&prefix, pos) else {
                        return Err(compile_error_str(
                            &format!(
                                "batch-impl: internal error — variadic segment `{}@..` \
                                 has no binding for element position {}",
                                prefix, pos,
                            ),
                            id.span(),
                        ));
                    };
                    out.extend(value.clone());
                    i += 2;
                    continue;
                }
                Some(TokenTree::Literal(lit)) => {
                    let Ok(n) = lit.to_string().parse::<usize>() else {
                        return Err(compile_error_str(
                            "batch-impl: `@` inside a repeat block must be followed by a \
                             segment name (`@ident`) or a number (`@0`)",
                            lit.span(),
                        ));
                    };
                    let val = Literal::u64_unsuffixed((n + round) as u64);
                    out.push(TokenTree::Literal(val));
                    i += 2;
                    continue;
                }
                _ => {
                    return Err(compile_error_str(
                        "batch-impl: `@` inside a repeat block must be followed by a \
                         segment name (`@ident`) or an index (`@N`)",
                        tokens[i].span(),
                    ));
                }
            }
        }
        if let TokenTree::Group(g) = &tokens[i] {
            if depth + 1 > MAX_NEST_DEPTH {
                return Err(depth_err(&tokens[i..i + 1], ""));
            }
            let inner = g.stream().into_iter().collect::<Vec<_>>();
            let substituted = substitute(&inner, cx, round, depth + 1)?;
            let mut ng = Group::new(g.delimiter(), substituted.into_iter().collect());
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
