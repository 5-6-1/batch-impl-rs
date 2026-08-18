//! Repeat-block expansion for impl bodies (Ext 2 variadic segments):
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
            // `@ ( body ) ..`
            if let Some(TokenTree::Group(g)) = tokens.get(i + 1)
                && g.delimiter() == delimiter![()]
                && is_punct_at(tokens, i + 2, '.')
                && is_punct_at(tokens, i + 3, '.')
            {
                let body = g.stream().into_iter().collect::<Vec<_>>();
                out.extend(expand_block(&body, segs, depth + 1)?);
                i += 4;
                continue;
            }
            return Err(compile_error_str(
                "batch-impl: `@` inside an impl body must start a repeat block `@(...)..`",
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
/// independent), then `L` rounds of marker substitution (`L` = the common
/// driving-segment length).
fn expand_block(
    body: &[TokenTree], segs: &[VarSeg], depth: usize,
) -> Result<Vec<TokenTree>, TokenStream> {
    if depth > MAX_NEST_DEPTH {
        return Err(depth_err(body, " in a repeat block"));
    }
    // 1. Nested repeat blocks expand first (own rounds).
    let body = expand_nested(body, segs, depth)?;
    // 2. Driving segments + the common length.
    let len = match collect_drivers(&body, segs)? {
        Some(l) => l,
        None => {
            return Err(compile_error_str(
                "batch-impl: a repeat block `@(...)..` must reference at least one \
                 variadic segment (`@ident`) to determine the repetition count",
                body.first().map_or_else(Span::call_site, |t| t.span()),
            ));
        }
    };
    // 3. L rounds of marker substitution.
    let mut out = vec![];
    for round in 0..len {
        out.extend(substitute(&body, segs, round, depth + 1)?);
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
        if is_punct_at(tokens, i, '@')
            && let Some(TokenTree::Group(g)) = tokens.get(i + 1)
            && g.delimiter() == delimiter![()]
            && is_punct_at(tokens, i + 2, '.')
            && is_punct_at(tokens, i + 3, '.')
        {
            let body = g.stream().into_iter().collect::<Vec<_>>();
            out.extend(expand_block(&body, segs, depth + 1)?);
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

/// The common length of the block's driving segments (`@ident` references,
/// deduplicated); `None` when the block references no segment at all.
fn collect_drivers(tokens: &[TokenTree], segs: &[VarSeg]) -> Result<Option<usize>, TokenStream> {
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
            let l = collect_drivers(&inner, segs)?;
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
    Ok(len)
}

/// Substitutes the markers of one round: `@ident` → the segment's i-th name,
/// `@N` → `N + i`.
fn substitute(
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

#[cfg(test)]
mod tests {
    use super::*;

    fn segs() -> Vec<VarSeg> {
        vec![
            VarSeg { prefix: "A".into(), start: 0, len: 3 },
            VarSeg { prefix: "B".into(), start: 1, len: 2 },
        ]
    }

    fn expand(s: &str) -> Result<String, String> {
        let ts = s.parse::<TokenStream>().map_err(|e| e.to_string())?;
        expand_repeat_blocks(ts, &segs()).map(|o| o.to_string()).map_err(|e| e.to_string())
    }

    #[test]
    fn single_segment_rounds() {
        assert_eq!(
            expand("@(@A::f(&self.@0),)..").unwrap(),
            "A0 :: f (& self .0) , A1 :: f (& self .1) , A2 :: f (& self .2) ,"
        );
    }

    #[test]
    fn offset_start_name_numbering() {
        // B starts at leaf index 1: names B1, B2; `@1` cursor → 1, 2.
        assert_eq!(
            expand("@(@B::f(&self.@1),)..").unwrap(),
            "B1 :: f (& self .1) , B2 :: f (& self .2) ,"
        );
    }

    #[test]
    fn multi_segment_parallel_rounds() {
        // Two equal-length segments drive the block: one shared cursor, each
        // round takes the i-th element of both.
        let segs = vec![
            VarSeg { prefix: "A".into(), start: 0, len: 2 },
            VarSeg { prefix: "B".into(), start: 2, len: 2 },
        ];
        let ts = "@(@A + @B,)..".parse::<TokenStream>().unwrap();
        let out = expand_repeat_blocks(ts, &segs).unwrap().to_string();
        assert_eq!(out, "A0 + B2 , A1 + B3 ,");
    }

    #[test]
    fn unequal_segment_lengths_error() {
        let segs = vec![
            VarSeg { prefix: "A".into(), start: 0, len: 3 },
            VarSeg { prefix: "B".into(), start: 1, len: 2 },
        ];
        let ts = "@(@A + @B,)..".parse::<TokenStream>().unwrap();
        assert!(expand_repeat_blocks(ts, &segs).is_err());
    }

    #[test]
    fn nested_cartesian() {
        // Outer rounds A0/A1/A2; each inner runs B over 1,2. The outer
        // block body has no trailing comma (the inner block's own trailing
        // commas separate the B elements), so no double comma appears.
        let out = expand("@(@A::f(&self.@0) @(@B::g(&self.@1),)..)..").unwrap();
        assert_eq!(
            out,
            "A0 :: f (& self .0) B1 :: g (& self .1) , B2 :: g (& self .2) , \
             A1 :: f (& self .1) B1 :: g (& self .1) , B2 :: g (& self .2) , \
             A2 :: f (& self .2) B1 :: g (& self .1) , B2 :: g (& self .2) ,"
        );
    }

    #[test]
    fn no_trailing_separator_concatenates() {
        assert_eq!(expand("@(@A)..").unwrap(), "A0 A1 A2");
    }

    #[test]
    fn float_literal_at_path_fixed() {
        // `self.0.@0` tokenizes `0.` as a float literal; the fix splits it
        // so the cursor expands into `self.0.0`, `self.0.1`, ...
        let segs = vec![VarSeg { prefix: "A".into(), start: 0, len: 2 }];
        let ts = "@(@A::from(self.0.@0),)..".parse::<TokenStream>().unwrap();
        let out = expand_repeat_blocks(ts, &segs).unwrap().to_string();
        assert_eq!(out, "A0 :: from (self . 0 . 0) , A1 :: from (self . 0 . 1) ,");
    }

    #[test]
    fn plain_body_passthrough() {
        let s = "fn combine (& self , rhs : & Self) -> Self { todo ! () }";
        assert_eq!(expand(s).unwrap(), s);
    }

    #[test]
    fn bare_at_errors() {
        assert!(expand("x @ 0").is_err());
    }

    #[test]
    fn unknown_segment_errors() {
        assert!(expand("@(@X::f(),)..").is_err());
    }

    #[test]
    fn no_driver_errors() {
        assert!(expand("@(@0,)..").is_err());
    }
}
