//! Variadic-segment marking: `ident@..` inside an `impl{...}` shape
//! template is a DSL-only construct (a variadic segment — not legal Rust),
//! so it must be replaced before the `@` constant stage (which would
//! otherwise reject the unknown `@`), before `angle_collect`, and before
//! the syn parse in codegen.
//!
//! The replacement is a placeholder ident encoding the segment's name
//! prefix: `__batch_varseg_{prefix}_{seq}` (seq disambiguates repeated
//! prefixes, which codegen rejects anyway). Codegen recovers the prefix
//! by stripping the reserved head and the trailing `_seq`.
//!
//! Only `impl{...}` template groups are entered (via
//! `util::is_impl_template`); every other Brace group stays passthrough —
//! bodies keep their `@` repeat-block markers untouched, and user constant
//! definitions at the top level are never scanned.

use proc_macro2::{Group, Ident, TokenStream, TokenTree};

use crate::util::{MAX_NEST_DEPTH, depth_err, is_impl_template, is_punct_at};

/// Reserved head of a variadic-segment placeholder ident.
pub(crate) const VARSEG_PREFIX: &str = "__batch_varseg_";

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
            let (marked, _) = mark_template(&inner, depth + 1, 0)?;
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
/// group (tuples `(A@..,)`, flat `<...>` args, arrays, ...). Returns the
/// marked stream and the next segment sequence number.
fn mark_template(
    tokens: &[TokenTree], depth: usize, mut seq: usize,
) -> Result<(Vec<TokenTree>, usize), TokenStream> {
    if depth > MAX_NEST_DEPTH {
        return Err(depth_err(tokens, ""));
    }
    let mut out = vec![];
    let mut i = 0;
    while i < tokens.len() {
        // `ident @ ..` — the `..` is two joint/alone `.` puncts; Spacing is
        // irrelevant (a leading segment never follows it). `@u8..u128` is
        // untouched: its `@` is not preceded by an ident.
        if let Some(TokenTree::Ident(id)) = tokens.get(i)
            && is_punct_at(tokens, i + 1, '@')
            && is_punct_at(tokens, i + 2, '.')
            && is_punct_at(tokens, i + 3, '.')
        {
            let placeholder = Ident::new(&format!("{}{}_{}", VARSEG_PREFIX, id, seq), id.span());
            out.push(TokenTree::Ident(placeholder));
            i += 4;
            seq += 1;
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
                    out.push(TokenTree::Punct(proc_macro2::Punct::new(',', proc_macro2::Spacing::Alone)));
                }
            }
            continue;
        }
        if let TokenTree::Group(g) = &tokens[i] {
            if depth + 1 > MAX_NEST_DEPTH {
                return Err(depth_err(&tokens[i..i + 1], ""));
            }
            let inner = g.stream().into_iter().collect::<Vec<_>>();
            let (marked, s) = mark_template(&inner, depth + 1, seq)?;
            seq = s;
            let mut ng = Group::new(g.delimiter(), marked.into_iter().collect());
            ng.set_span(g.span());
            out.push(TokenTree::Group(ng));
            i += 1;
            continue;
        }
        out.push(tokens[i].clone());
        i += 1;
    }
    Ok((out, seq))
}

/// Whether a syn type is a variadic-segment placeholder (a bare ident with
/// the reserved head).
pub(crate) fn is_varseg_type(tp: &syn::Type) -> bool {
    let syn::Type::Path(p) = tp else { return false };
    p.qself.is_none()
        && p.path.segments.len() == 1
        && matches!(p.path.segments[0].arguments, syn::PathArguments::None)
        && p.path.segments[0].ident.to_string().starts_with(VARSEG_PREFIX)
}

/// The variadic segment's name prefix from its placeholder ident
/// (`__batch_varseg_A_0` → `A`; `__batch_varseg_Foo_Bar_3` → `Foo_Bar`).
pub(crate) fn varseg_prefix(ident: &Ident) -> Option<String> {
    let s = ident.to_string();
    let rest = s.strip_prefix(VARSEG_PREFIX)?;
    let (prefix, _seq) = rest.rsplit_once('_')?;
    (!prefix.is_empty()).then(|| prefix.to_string())
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
        assert_eq!(mark("impl{(A@..,)}"), "impl { (__batch_varseg_A_0 ,) }");
    }

    #[test]
    fn trailing_segment_without_comma() {
        // `impl{(A@..)}` — no trailing comma: the marker supplies one so the
        // template still parses as a tuple, not a parenthesized group.
        assert_eq!(mark("impl{(A@..)}"), "impl { (__batch_varseg_A_0 ,) }");
    }

    #[test]
    fn fixed_then_trailing_segment_without_comma() {
        assert_eq!(mark("impl{(u8, A@..)}"), "impl { (u8 , __batch_varseg_A_0 ,) }");
    }

    #[test]
    fn middle_segment_keeps_stream_comma() {
        // `(A@.., B@..)` — the first segment is followed by the real comma.
        assert_eq!(
            mark("impl{(A@.., B@..)}"),
            "impl { (__batch_varseg_A_0 , __batch_varseg_B_1 ,) }"
        );
    }

    #[test]
    fn fixed_before_segment() {
        assert_eq!(mark("impl{(u8, B@..,)}"), "impl { (u8 , __batch_varseg_B_0 ,) }");
    }

    #[test]
    fn nested_tuples() {
        // The seq counter is template-global: B is the second segment.
        assert_eq!(
            mark("impl{((A@..,),(B@..,))}"),
            "impl { ((__batch_varseg_A_0 ,) , (__batch_varseg_B_1 ,)) }"
        );
    }

    #[test]
    fn angle_args_marked() {
        // flat `<...>` inside the template (angle_collect has not run yet)
        assert_eq!(mark("impl{Vec<(A@..,)>}"), "impl { Vec < (__batch_varseg_A_0 ,) > }");
    }

    #[test]
    fn repeated_prefix_seq() {
        assert_eq!(
            mark("impl{(A@.., A@..,)}"),
            "impl { (__batch_varseg_A_0 , __batch_varseg_A_1 ,) }"
        );
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
    fn prefix_roundtrip() {
        for (ph, expect) in [
            ("__batch_varseg_A_0", "A"),
            ("__batch_varseg_Foo_Bar_3", "Foo_Bar"),
            ("__batch_varseg_u8_0", "u8"),
        ] {
            let id = Ident::new(ph, proc_macro2::Span::call_site());
            assert_eq!(varseg_prefix(&id).as_deref(), Some(expect), "{ph}");
        }
        assert_eq!(varseg_prefix(&Ident::new("plain", proc_macro2::Span::call_site())), None);
    }
}
