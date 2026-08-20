//! Repeat-block driver collection and marker substitution: the two helpers
//! of the repeat-block expansion (`expand_repeat_blocks` in `repeat.rs`).
//! `collect_drivers` finds the `@ident` segment references of a block body
//! and their common length; `substitute` rewrites the markers of one round
//! (`@ident` → the segment's i-th name, `@N` → `N + i`).

use proc_macro2::{Group, Ident, Literal, TokenStream, TokenTree};

use crate::codegen::VarSeg;
use crate::util::{MAX_NEST_DEPTH, compile_error_str, depth_err, is_punct_at};

/// The inner segment references of a block body: the deduplicated `@ident`
/// prefixes (first-appearance order) and their common length (`None` when the
/// block references no segment — a cursor-only block).
pub(crate) fn collect_drivers(
    tokens: &[TokenTree], segs: &[VarSeg],
) -> Result<(Vec<String>, Option<usize>), TokenStream> {
    let mut prefixes: Vec<String> = vec![];
    let mut len: Option<usize> = None;
    let mut i = 0;
    while i < tokens.len() {
        if is_punct_at(tokens, i, '@') {
            // `@N` index cursors are not segment references — skip them.
            if matches!(tokens.get(i + 1), Some(TokenTree::Literal(_))) {
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

/// Substitutes the markers of one round: `@ident` → the segment's i-th name,
/// `@N` → `N + i`.
pub(crate) fn substitute(
    tokens: &[TokenTree], segs: &[VarSeg], round: usize, depth: usize,
) -> Result<Vec<TokenTree>, TokenStream> {
    if depth > MAX_NEST_DEPTH {
        return Err(depth_err(tokens, ""));
    }
    let mut out = vec![];
    let mut i = 0;
    while i < tokens.len() {
        if is_punct_at(tokens, i, '@') {
            match tokens.get(i + 1) {
                Some(TokenTree::Ident(id)) => {
                    let prefix = id.to_string();
                    let Some(seg) = segs.iter().find(|s| s.prefix == prefix) else {
                        // Verified by collect_drivers; defensive (no-panic).
                        return Err(compile_error_str(
                            &format!("batch-impl: unknown variadic segment `@{}`", prefix),
                            id.span(),
                        ));
                    };
                    let name = Ident::new(&format!("{}{}", prefix, seg.start + round), id.span());
                    out.push(TokenTree::Ident(name));
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
            let substituted = substitute(&inner, segs, round, depth + 1)?;
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
