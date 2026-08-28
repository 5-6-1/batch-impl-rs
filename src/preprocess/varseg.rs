//! Variadic-segment marking: `ident@..` inside an `impl{...}` shape
//! template is a DSL-only construct (a variadic segment — not legal Rust),
//! so it must be replaced before the `@` constant stage (which would
//! otherwise reject the unknown `@`), before `angle_collect`, and before
//! the syn parse in codegen.
//!
//! The replacement is a **structural marker**, not a reserved name: an
//! array type `[Prefix; ()]` — unit-tuple length. It parses as
//! ordinary Rust (so `syn` accepts the template), and codegen decodes it by
//! shape; the two stacked features never co-occur in a meaningful template,
//! recovering the segment's name prefix from the element position. No
//! reserved identifier pattern exists anywhere in the pipeline.
//!
//! Only `impl{...}` template groups are entered (via
//! `util::is_impl_template`); every other Brace group stays passthrough —
//! bodies keep their `@` repeat-block markers untouched, and user constant
//! definitions at the top level are never scanned.
use proc_macro2::{Group, TokenStream, TokenTree};

use crate::util::{MAX_NEST_DEPTH, depth_err, is_impl_template, is_punct_at};

/// Replaces every `ident@..` in `impl{...}` templates with a placeholder.
pub(crate) fn mark_varseg(tokens: &[TokenTree]) -> Result<Vec<TokenTree>, TokenStream> {
    mark_varseg_at(tokens, 0)
}

fn mark_varseg_at(tokens: &[TokenTree], depth: usize) -> Result<Vec<TokenTree>, TokenStream> {
    if depth > MAX_NEST_DEPTH {
        return Err(depth_err(tokens, ""));
    }
    let mut out = vec![];
    let mut i = 0;
    while i < tokens.len() {
        // `impl{...}` shape template: enter and mark its segments.
        if is_impl_template(tokens, i)
            && let Some(TokenTree::Group(g)) = tokens.get(i + 1)
            && g.delimiter() == delimiter![{}]
        {
            let inner = g.stream().into_iter().collect::<Vec<_>>();
            let marked = mark_template(&inner, depth + 1)?;
            let mut ng = Group::new(delimiter![{}], marked.into_iter().collect());
            ng.set_span(g.span());
            out.push(tokens[i].clone());
            out.push(TokenTree::Group(ng));
            i += 2;
            continue;
        }
        // Paren / Bracket / transparent groups recurse (a template may nest
        // inside a list or a `.` argument); Brace bodies stay passthrough.
        if let TokenTree::Group(g) = &tokens[i]
            && g.delimiter() != delimiter![{}]
        {
            if crate::util::bracket_is_passthrough(tokens, i) {
                out.push(tokens[i].clone());
            } else {
                if depth + 1 > MAX_NEST_DEPTH {
                    return Err(depth_err(&tokens[i..i + 1], ""));
                }
                let inner = g.stream().into_iter().collect::<Vec<_>>();
                let mut ng = Group::new(
                    g.delimiter(),
                    mark_varseg_at(&inner, depth + 1)?.into_iter().collect(),
                );
                ng.set_span(g.span());
                out.push(TokenTree::Group(ng));
            }
            i += 1;
            continue;
        }
        out.push(tokens[i].clone());
        i += 1;
    }
    Ok(out)
}

/// Marks `ident@..` inside one template's token stream; recurses into every
/// group (tuples `(A@..,)`, flat `<...>` args, arrays, ...). Shared by
/// `mark_varseg` (inside `impl{...}` groups) and the impl entry's shape
/// template (`(T@..)` — the template of the shape form).
pub(crate) fn mark_template(
    tokens: &[TokenTree], depth: usize,
) -> Result<Vec<TokenTree>, TokenStream> {
    if depth > MAX_NEST_DEPTH {
        return Err(depth_err(tokens, ""));
    }
    let out = mark_template_impl(tokens, depth)?;
    // Postcondition canary: `mark_template`'s contract is to consume every
    // `ident@..` inside the template — its output must contain none. This
    // guard lives **here** (the consumer's output), not at `expand_consts`'s
    // input, because only here is the `ident@..` shape unambiguous: an open
    // constant range (`@..u128`) has its `@` preceded by `<`/`,`/`(` — never
    // an ident — while a true segment is `ident @ ..`; at `expand_consts`'s
    // input the same shape is a legal error path (`A@..` reports "range
    // constant must name endpoint") and must not panic. A mis-ordered or
    // partial marking therefore surfaces here as a loud debug panic, and
    // fuzz's direct calls are covered (they pass through this function).
    debug_assert!(
        !contains_unmarked_segment(&out),
        "mark_template: output still contains an unmarked `ident@..` \
         (variadic-segment marking failed to consume it)"
    );
    Ok(out)
}

/// The segment shape `mark_template` consumes: `ident @ . .` (an ident
/// **before** the `@`). Single authority for the shape — the marking loop
/// tests it at each position, and the postcondition canary re-checks the
/// output with the same predicate, so the two cannot drift (the previous
/// regression root cause was detection and consumption disagreeing).
fn is_unmarked_segment_at(tokens: &[TokenTree], i: usize) -> bool {
    matches!(&tokens[i], TokenTree::Ident(_))
        && is_punct_at(tokens, i + 1, '@')
        && is_punct_at(tokens, i + 2, '.')
        && is_punct_at(tokens, i + 3, '.')
}

/// Whether `tokens` contains any unmarked segment (the postcondition test).
fn contains_unmarked_segment(tokens: &[TokenTree]) -> bool {
    (0..tokens.len().saturating_sub(3)).any(|i| is_unmarked_segment_at(tokens, i))
}

/// The marking loop (see [`mark_template`]); recurses into groups.
fn mark_template_impl(tokens: &[TokenTree], depth: usize) -> Result<Vec<TokenTree>, TokenStream> {
    let mut out = vec![];
    let mut i = 0;
    while i < tokens.len() {
        // `ident @ ..` — the `..` is two joint/alone `.` puncts; Spacing is
        // irrelevant (a leading segment never follows it). `@u8..u128` is
        // untouched: its `@` is not preceded by an ident. The shape test is
        // the shared [`is_unmarked_segment_at`] — the postcondition canary
        // re-checks with the same predicate.
        if is_unmarked_segment_at(tokens, i) {
            // Structural marker: `[Prefix; ()]` — an array whose length is
            // the **unit tuple**: a shape that cannot appear in any
            // compilable code (array lengths are `usize`; `()` never is), so
            // no meaningful user template can collide with it. Ordinary Rust
            // for `syn`, decoded by shape — no reserved name involved.
            let Some(TokenTree::Ident(id)) = tokens.get(i) else {
                // `is_unmarked_segment_at` guarantees an ident at `i`; the
                // else-branch exists only to keep the no-panic-on-input
                // promise (skip the position rather than `unreachable!`).
                i += 1;
                continue;
            };
            let mut marker: TokenStream = TokenTree::Ident(id.clone()).into();
            marker.extend(std::iter::once(TokenTree::Punct(proc_macro2::Punct::new(
                ';',
                proc_macro2::Spacing::Alone,
            ))));
            marker.extend(std::iter::once(TokenTree::Group(Group::new(
                delimiter![()],
                TokenStream::new(),
            ))));
            out.push(TokenTree::Group(Group::new(delimiter![[]], marker)));
            i += 4;
            // A variadic segment at the end of a tuple/list element list is
            // normally written with a trailing comma (`(A@..,)`) — the comma
            // keeps syn from parsing `(A@..)` as a parenthesized group. When
            // the user omits it (`impl{(A@..)}`), supply it here so the
            // template still parses as a tuple. Only the *last* element of
            // the enclosing group needs this — a middle segment already has
            // the following comma in the stream.
            if i >= tokens.len() && !out.is_empty() {
                let last_is_comma = matches!(
                    out.last(),
                    Some(TokenTree::Punct(p)) if p.as_char() == ','
                );
                if !last_is_comma {
                    out.push(TokenTree::Punct(proc_macro2::Punct::new(
                        ',',
                        proc_macro2::Spacing::Alone,
                    )));
                }
            }
            continue;
        }
        if let TokenTree::Group(g) = &tokens[i] {
            if depth + 1 > MAX_NEST_DEPTH {
                return Err(depth_err(&tokens[i..i + 1], ""));
            }
            let inner = g.stream().into_iter().collect::<Vec<_>>();
            // Inner recursion: use the raw impl (the public entry's
            // postcondition runs once at the template's top level).
            let marked = mark_template_impl(&inner, depth + 1)?;
            let mut ng = Group::new(g.delimiter(), marked.into_iter().collect());
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

/// Whether a syn type is a variadic-segment marker: an array type whose
/// length is the **unit tuple** (`[Prefix; ()]`) — a shape that cannot
/// appear in compilable code, decoded by shape, never by a name pattern.
pub(crate) fn is_varseg_type(tp: &syn::Type) -> bool {
    let syn::Type::Array(a) = tp else { return false };
    is_varseg_array(a)
}

/// Whether a syn array type is a variadic-segment marker (`[A; ()]` — the
/// unit-tuple length). Same test as [`is_varseg_type`] on the array form.
pub(crate) fn is_varseg_array(a: &syn::TypeArray) -> bool {
    is_unit_len(&a.len) && varseg_prefix(&syn::Type::Array(a.clone())).is_some()
}

/// The variadic segment's name prefix from its marker (`[A; ()]` → `A`; the
/// element must be a bare single-segment path and the length the unit tuple).
pub(crate) fn varseg_prefix(tp: &syn::Type) -> Option<String> {
    let syn::Type::Array(a) = tp else { return None };
    // The length must be the unit tuple — the marker's distinguishing shape.
    if !is_unit_len(&a.len) {
        return None;
    }
    let syn::Type::Path(p) = &*a.elem else { return None };
    if !p.qself.is_none() || p.path.segments.len() != 1 {
        return None;
    }
    matches!(p.path.segments[0].arguments, syn::PathArguments::None)
        .then(|| p.path.segments[0].ident.to_string())
}

/// Whether an expression is an empty tuple literal — `()`, the marker's
/// length shape (an array length of `()` cannot exist in compiled code).
fn is_unit_len(expr: &syn::Expr) -> bool {
    matches!(expr, syn::Expr::Tuple(t) if t.elems.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mark(s: &str) -> String {
        let v = s.parse::<TokenStream>().unwrap().into_iter().collect::<Vec<_>>();
        mark_varseg(&v).unwrap().into_iter().collect::<TokenStream>().to_string()
    }

    #[test]
    fn top_level_segment_marked() {
        assert_eq!(mark("impl{(A@..,)}"), "impl { ([A ; ()] ,) }");
    }

    #[test]
    fn trailing_segment_without_comma() {
        // `impl{(A@..)}` — no trailing comma: the marker supplies one so the
        // template still parses as a tuple, not a parenthesized group.
        assert_eq!(mark("impl{(A@..)}"), "impl { ([A ; ()] ,) }");
    }

    #[test]
    fn fixed_then_trailing_segment_without_comma() {
        assert_eq!(mark("impl{(u8, A@..)}"), "impl { (u8 , [A ; ()] ,) }");
    }

    #[test]
    fn middle_segment_keeps_stream_comma() {
        // `(A@.., B@..)` — the first segment is followed by the real comma.
        assert_eq!(mark("impl{(A@.., B@..)}"), "impl { ([A ; ()] , [B ; ()] ,) }");
    }

    #[test]
    fn fixed_before_segment() {
        assert_eq!(mark("impl{(u8, B@..,)}"), "impl { (u8 , [B ; ()] ,) }");
    }

    #[test]
    fn nested_tuples() {
        assert_eq!(mark("impl{((A@..,),(B@..,))}"), "impl { (([A ; ()] ,) , ([B ; ()] ,)) }");
    }

    #[test]
    fn angle_args_marked() {
        // flat `<...>` inside the template (angle_collect has not run yet)
        assert_eq!(mark("impl{Vec<(A@..,)>}"), "impl { Vec < ([A ; ()] ,) > }");
    }

    #[test]
    fn repeated_prefix_both_marked() {
        // Repeated prefixes are rejected later (duplicate-prefix check in
        // the shape match) — marking itself is uniform.
        assert_eq!(mark("impl{(A@.., A@..,)}"), "impl { ([A ; ()] , [A ; ()] ,) }");
    }

    #[test]
    fn body_untouched() {
        // Brace bodies are never entered: the `@` repeat-block markers stay.
        assert_eq!(
            mark("T #combine{@(@A::f(&self.@0),)..}"),
            "T # combine { @ (@ A :: f (& self .@ 0) ,) .. }"
        );
    }

    #[test]
    fn const_ranges_untouched() {
        // `@u8..u128` is a constant range, not a segment: its `@` has no
        // ident before it.
        assert_eq!(mark("impl{[@u8..u128]}"), "impl { [@ u8 .. u128] }");
    }

    #[test]
    fn prefix_roundtrip_by_shape() {
        for (ph, expect) in [("[A ; ()]", "A"), ("[Foo_Bar ; ()]", "Foo_Bar"), ("[u8 ; ()]", "u8")]
        {
            let ts: TokenStream = ph.parse().unwrap();
            let ty: syn::Type = syn::parse2(ts).unwrap();
            assert_eq!(varseg_prefix(&ty).as_deref(), Some(expect), "{ph}");
            assert!(is_varseg_type(&ty), "{ph}");
        }
        // A real element type: an ordinary array template, never a segment.
        let plain: syn::Type = syn::parse_str("[A; 3]").unwrap();
        assert!(!is_varseg_type(&plain));
        assert_eq!(varseg_prefix(&plain), None);
    }
}
