//! Variadic-segment marking: `P@..` inside an `impl{...}` shape
//! template is a DSL-only construct (a variadic segment — not legal Rust),
//! so it must be replaced before the `@` constant stage (which would
//! otherwise reject the unknown `@`), before `angle_collect`, and before
//! the syn parse in codegen.
//!
//! The replacement is a **structural marker**, not a reserved name: an
//! array type `[P; ()]` — unit-tuple length. It parses as
//! ordinary Rust (so `syn` accepts the template), and codegen decodes it by
//! shape; the two stacked features never co-occur in a meaningful template,
//! recovering the complete Rust type pattern from the element position. No
//! reserved identifier pattern exists anywhere in the pipeline.
//!
//! Only `impl{...}` template groups are entered (via
//! `util::is_impl_template`); every other Brace group stays passthrough —
//! bodies keep their `@` repeat-block markers untouched, and user constant
//! definitions at the top level are never scanned.

use proc_macro2::{Group, TokenStream, TokenTree};

use crate::util::{
    MAX_NEST_DEPTH, Op, TokenDomain, compile_error_str, depth_err, group_at, is_impl_template,
    is_punct_at, read_op, slice_upto,
};

/// Replaces every `P@..` in `impl{...}` templates with a structural marker.
pub(crate) fn mark_varseg(tokens: &[TokenTree]) -> Result<Vec<TokenTree>, TokenStream> {
    mark_varseg_at(tokens, 0)
}

fn mark_varseg_at(tokens: &[TokenTree], depth: usize) -> Result<Vec<TokenTree>, TokenStream> {
    if depth > MAX_NEST_DEPTH {
        return Err(depth_err(tokens, ""));
    }
    let mut out = vec![];
    let mut i = 0;
    while let Some(cur) = tokens.get(i) {
        // `impl{...}` shape template: enter and mark its segments.
        if is_impl_template(tokens, i)
            && let Some(TokenTree::Group(g)) = tokens.get(i + 1)
            && g.delimiter() == delimiter![{}]
        {
            let inner = g.stream().into_iter().collect::<Vec<_>>();
            let marked = mark_template(&inner, depth + 1)?;
            let mut ng = Group::new(delimiter![{}], marked.into_iter().collect());
            ng.set_span(g.span());
            out.push(cur.clone());
            out.push(TokenTree::Group(ng));
            i += 2;
            continue;
        }
        // Paren / Bracket / transparent groups recurse (a template may nest
        // inside a list or a `.` argument); Brace bodies stay passthrough.
        if let TokenTree::Group(g) = cur
            && g.delimiter() != delimiter![{}]
        {
            if crate::util::bracket_is_passthrough(tokens, i) {
                out.push(cur.clone());
            } else {
                if depth + 1 > MAX_NEST_DEPTH {
                    return Err(depth_err(std::slice::from_ref(cur), ""));
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
        out.push(cur.clone());
        i += 1;
    }
    Ok(out)
}

/// Marks `P@..` inside one template's type structure (tuples `(A@..,)`,
/// flat `<...>` args, arrays, ...). Ordinary braces, macros and attributes
/// remain opaque. Shared by
/// `mark_varseg` (inside `impl{...}` groups) and the impl entry's shape
/// template (`(T@..)` — the template of the shape form).
pub(crate) fn mark_template(
    tokens: &[TokenTree], depth: usize,
) -> Result<Vec<TokenTree>, TokenStream> {
    if depth > MAX_NEST_DEPTH {
        return Err(depth_err(tokens, ""));
    }
    let out = mark_template_impl(tokens, depth, false)?;
    // Postcondition: `mark_template`'s contract is to consume every
    // visible type-position `P@..` inside the template — its output must
    // contain none. Opaque Rust inputs may retain the same tokens. An open
    // constant range (`@..u128`) starts a slot rather than following a type,
    // so it is not a segment. Detection and rewriting share SlotBoundary.
    //
    // It reports instead of asserting: a panic inside a proc macro is a
    // compiler ICE, and the no-panic promise is unconditional. The invariant
    // is structural (the loop consumes every shape, and no transformation can
    // create one — swept exhaustively in the test module), so this branch is
    // unreachable for user input; keeping it a diagnostic rather than a
    // `debug_assert!` means a future regression degrades to a clear error in
    // every build profile instead of a user-visible panic in debug ones.
    if let Some(left) = first_unmarked_segment(&out) {
        let span = left.first().map_or_else(proc_macro2::Span::call_site, |t| t.span());
        return Err(compile_error_str(
            "batch-impl: internal error: variadic-segment marking left an \
             unmarked `ident@..` (please report this spelling)",
            span,
        ));
    }
    Ok(out)
}

/// Boundaries of complete type slots, tracked forwards before angle pairing.
/// `fn(T) -> U` stays one slot; `<...>` starts its own comma-separated slots.
/// A named function argument or associated binding starts its type after
/// `:` / `=`. Path separators and return arrows never split a type slot.
#[derive(Default)]
struct SlotBoundary {
    start: usize,
    angles: Vec<usize>,
}

impl SlotBoundary {
    fn has_suffix(&self, tokens: &[TokenTree], i: usize, position: usize) -> bool {
        self.start < position
            && is_punct_at(tokens, i, '@')
            && is_punct_at(tokens, i + 1, '.')
            && is_punct_at(tokens, i + 2, '.')
    }

    /// `position` is measured in the current output, or input for read-only
    /// detection. Return the token count so compound operators stay atomic.
    fn advance(&mut self, tokens: &[TokenTree], i: usize, position: usize) -> usize {
        if is_punct_at(tokens, i, '#') {
            let group = i + 1 + usize::from(is_punct_at(tokens, i + 1, '!'));
            if group_at(tokens, group, delimiter![[]]).is_some() {
                let count = group + 1 - i;
                if self.start == position {
                    self.start += count;
                }
                return count;
            }
        }
        if is_punct_at(tokens, i, '<') {
            self.angles.push(self.start);
            self.start = position + 1;
        } else if is_punct_at(tokens, i, '>') {
            if let Some(parent) = self.angles.pop() {
                self.start = parent;
            }
        } else if let Some((op, count)) = read_op(tokens, i) {
            if matches!(op, Op::Comma | Op::Semicolon | Op::Colon | Op::Eq) {
                self.start = position + count;
            }
            return count;
        }
        1
    }
}

/// Array lengths are expression tokens, not type slots. This is the same
/// boundary as the shared type-token scanner; const/macro groups stay opaque.
fn type_group_len(group: &Group, tokens: &[TokenTree]) -> usize {
    if group.delimiter() == delimiter![[]] {
        tokens
            .iter()
            .position(|t| matches!(t, TokenTree::Punct(p) if p.as_char() == ';'))
            .unwrap_or(tokens.len())
    } else {
        tokens.len()
    }
}

/// The first surviving type-position `P@..`, nested type groups included.
/// The postcondition shares the marking loop's domain: ordinary braces,
/// macros and attributes may retain these tokens. A partial marking that
/// skipped a nested type would otherwise slip past a top-level-only test.
/// Returns the offending slice (for the diagnostic's span).
fn first_unmarked_segment(tokens: &[TokenTree]) -> Option<Vec<TokenTree>> {
    let mut slot = SlotBoundary::default();
    let mut i = 0;
    while let Some(t) = tokens.get(i) {
        if slot.has_suffix(tokens, i, i) {
            return Some(tokens.to_vec());
        }
        if let TokenTree::Group(g) = t
            && !TokenDomain::Type.skips_group(tokens, i)
        {
            let inner = g.stream().into_iter().collect::<Vec<_>>();
            if let Some(found) =
                first_unmarked_segment(slice_upto(&inner, type_group_len(g, &inner)))
            {
                return Some(found);
            }
        }
        i += slot.advance(tokens, i, i);
    }
    None
}

/// The marking loop and [`first_unmarked_segment`] enter the same type
/// groups. Rust patterns such as `n @ ..=3` in a const block, and arbitrary
/// macro/attribute inputs, are not variadic segments. Both walkers must stop
/// at these boundaries or legal Rust may be rewritten or report a residue.
fn mark_template_impl(
    tokens: &[TokenTree], depth: usize, tuple_members: bool,
) -> Result<Vec<TokenTree>, TokenStream> {
    let mut out = vec![];
    let mut slot = SlotBoundary::default();
    let mut i = 0;
    while let Some(cur) = tokens.get(i) {
        if slot.has_suffix(tokens, i, out.len()) {
            // The whole current slot is P, including modifiers, function
            // returns and nested generics. Its nested groups are already
            // marked. No backward type-grammar guess or name is involved.
            let mut marker = out.split_off(slot.start).into_iter().collect::<TokenStream>();
            marker.extend(std::iter::once(TokenTree::Punct(proc_macro2::Punct::new(
                ';',
                proc_macro2::Spacing::Alone,
            ))));
            marker.extend(std::iter::once(TokenTree::Group(Group::new(
                delimiter![()],
                TokenStream::new(),
            ))));
            let mut group = Group::new(delimiter![[]], marker);
            group.set_span(cur.span());
            out.push(TokenTree::Group(group));
            i += 3;
            // A variadic segment at the end of a parenthesized member list is
            // normally written with a trailing comma (`(A@..,)`) — the comma
            // keeps syn from parsing `(A@..)` as a parenthesized group. When
            // the user omits it (`impl{(A@..)}`), supply it here so the
            // template still parses as a tuple. Only the *last* element of
            // the enclosing group needs this — a middle segment already has
            // the following comma in the stream.
            if tuple_members && i >= tokens.len() {
                out.push(TokenTree::Punct(proc_macro2::Punct::new(
                    ',',
                    proc_macro2::Spacing::Alone,
                )));
            }
            continue;
        }
        if let TokenTree::Group(g) = cur
            && !TokenDomain::Type.skips_group(tokens, i)
        {
            if depth + 1 > MAX_NEST_DEPTH {
                return Err(depth_err(std::slice::from_ref(cur), ""));
            }
            let inner = g.stream().into_iter().collect::<Vec<_>>();
            // Inner recursion: use the raw impl (the public entry's
            // postcondition runs once at the template's top level).
            let end = type_group_len(g, &inner);
            let mut marked = mark_template_impl(
                slice_upto(&inner, end),
                depth + 1,
                g.delimiter() == delimiter![()],
            )?;
            marked.extend(inner.into_iter().skip(end));
            let mut ng = Group::new(g.delimiter(), marked.into_iter().collect());
            ng.set_span(g.span());
            out.push(TokenTree::Group(ng));
            i += 1;
            continue;
        }
        let count = slot.advance(tokens, i, out.len());
        out.extend(tokens.iter().skip(i).take(count).cloned());
        i += count;
    }
    Ok(out)
}

/// Whether a syn type is a variadic-segment marker: an array type whose
/// length is the **unit tuple** (`[P; ()]`) — a shape that cannot
/// appear in compilable code, decoded by shape, never by a name pattern.
#[cfg(test)]
pub(crate) fn is_varseg_type(tp: &syn::Type) -> bool {
    let syn::Type::Array(a) = tp else { return false };
    is_varseg_array(a)
}

/// Whether a syn array type is a variadic-segment marker (`[A; ()]` — the
/// unit-tuple length). Same test as [`is_varseg_type`] on the array form.
pub(crate) fn is_varseg_array(a: &syn::TypeArray) -> bool {
    is_unit_len(&a.len)
}

/// The complete Rust type pattern carried by a segment marker.
pub(crate) fn varseg_pattern(tp: &syn::Type) -> Option<&syn::Type> {
    let syn::Type::Array(a) = tp else { return None };
    is_varseg_array(a).then_some(a.elem.as_ref())
}

/// The variadic segment's name prefix from its marker (`[A; ()]` → `A`; the
/// element must be a bare single-segment path and the length the unit tuple).
#[cfg(test)]
pub(crate) fn varseg_prefix(tp: &syn::Type) -> Option<String> {
    let syn::Type::Path(p) = varseg_pattern(tp)? else { return None };
    if !p.qself.is_none() || p.path.segments.len() != 1 {
        return None;
    }
    let seg = p.path.segments.first()?;
    matches!(seg.arguments, syn::PathArguments::None).then(|| seg.ident.to_string())
}

/// Whether an expression is an empty tuple literal — `()`, the marker's
/// length shape (an array length of `()` cannot exist in compiled code).
fn is_unit_len(expr: &syn::Expr) -> bool {
    matches!(expr, syn::Expr::Tuple(t) if t.elems.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::{Group, Ident, Literal, Punct, Spacing, TokenStream};
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::str::FromStr;

    fn mark(s: &str) -> String {
        let v = s.parse::<TokenStream>().unwrap().into_iter().collect::<Vec<_>>();
        mark_varseg(&v).unwrap().into_iter().collect::<TokenStream>().to_string()
    }

    /// Recursive residue test — the canary's contract ("the output contains
    /// no type-position `ident@..`") checked at every visible type level. Uses the
    /// production detector (single authority for the shape).
    fn residue(tokens: &[TokenTree]) -> Option<Vec<TokenTree>> {
        first_unmarked_segment(tokens)
    }

    fn seg_tokens() -> Vec<TokenTree> {
        vec![
            Ident::new("A", proc_macro2::Span::call_site()).into(),
            Punct::new('@', Spacing::Alone).into(),
            Punct::new('.', Spacing::Alone).into(),
            Punct::new('.', Spacing::Alone).into(),
        ]
    }

    /// The residue postcondition (`mark_template`) must **never** fire on any
    /// input: it used to be a `debug_assert!` — a false positive was a
    /// user-visible compiler panic in debug builds (proc macros usually build
    /// debug) — and is now a diagnostic, where a false positive would still be
    /// a defect (a legal spelling rejected as an internal error). The
    /// invariant is structural (the loop consumes every visible `ident@..`; no
    /// transformation can create one), so this sweeps **every** sequence up
    /// to length 6 over an alphabet that covers the shape's token kinds,
    /// separators, and nested groups — then randomizes longer sequences.
    #[test]
    fn postcondition_canary_never_fires() {
        let alpha: Vec<TokenTree> = vec![
            Ident::new("A", proc_macro2::Span::call_site()).into(),
            Ident::new("B", proc_macro2::Span::call_site()).into(),
            Punct::new('@', Spacing::Alone).into(),
            Punct::new('.', Spacing::Alone).into(),
            Punct::new(',', Spacing::Alone).into(),
            Literal::from_str("0").unwrap().into(),
            Group::new(delimiter![()], seg_tokens().into_iter().collect()).into(),
            Group::new(delimiter![{}], seg_tokens().into_iter().collect()).into(),
        ];
        let silent = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let mut failures: Vec<String> = vec![];
        let mut check = |seq: &[TokenTree]| {
            let outcome = catch_unwind(AssertUnwindSafe(|| mark_template(seq, 0)));
            match outcome {
                // A panic anywhere must not exist in this path.
                Err(_) => failures.push(format!("PANIC on {seq:?}")),
                Ok(Ok(out)) => {
                    if let Some(left) = residue(&out) {
                        failures.push(format!("RESIDUE {left:?} from {seq:?}"));
                    }
                }
                // The internal-error diagnostic must be unreachable: reaching
                // it means the invariant broke and a user would get an error
                // instead of an expansion.
                Ok(Err(e)) => {
                    if e.to_string().contains("internal error") {
                        failures.push(format!("CANARY FIRED on {seq:?}"));
                    }
                }
            }
        };
        // Exhaustive: every sequence of length 0..=6 (8^6 + ... ≈ 300k).
        for len in 0..=6usize {
            let total = alpha.len().pow(len as u32);
            for n in 0..total {
                let mut idx = n;
                let mut seq = Vec::with_capacity(len);
                for _ in 0..len {
                    seq.push(alpha[idx % alpha.len()].clone());
                    idx /= alpha.len();
                }
                check(&seq);
            }
        }
        // Randomized longer sequences (xorshift — deterministic, no deps):
        // overlapping and repeated shapes beyond the exhaustive length.
        let mut state = 0x2545_F491_4F6C_DD1Du64;
        for _ in 0..20_000 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let len = (state % 24) as usize;
            let mut seq = Vec::with_capacity(len);
            for _ in 0..len {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                seq.push(alpha[(state % alpha.len() as u64) as usize].clone());
            }
            check(&seq);
        }
        std::panic::set_hook(silent);
        assert!(failures.is_empty(), "residue canary fired: {failures:#?}");
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
    fn template_const_patterns_remain_rust_tokens() {
        let source = "impl{Arr<{ match 2 { n @ ..=3 => n, _ => 0 } }>}";
        let unchanged = source.parse::<TokenStream>().unwrap().to_string();
        assert_eq!(mark(source), unchanged);
    }

    #[test]
    fn template_opaque_groups_are_not_segment_residue() {
        for source in [
            "impl{opaque!(A@..)}",
            "impl{opaque![A@..]}",
            "impl{opaque!{A@..}}",
            "impl{fn(#[opaque(A@..)] u8)}",
            "impl{Arr<{ match 2 { n @ ..=3 => n, _ => 0 } }>} ",
        ] {
            let tokens = source.parse::<TokenStream>().unwrap();
            assert!(residue(&tokens.clone().into_iter().collect::<Vec<_>>()).is_none());
            assert_eq!(mark(source), tokens.to_string());
        }
    }

    #[test]
    fn nested_type_segments_and_opaque_groups_share_one_template() {
        let source = "impl{Wrap<((A@..,),), opaque!(B@..),
            Arr<{ match 2 { n @ ..=3 => n, _ => 0 } }>, fn(#[opaque(C@..)] u8)>}";
        let expected = "impl{Wrap<(([A; ()],),), opaque!(B@..),
            Arr<{ match 2 { n @ ..=3 => n, _ => 0 } }>, fn(#[opaque(C@..)] u8)>}";
        let input = source.parse::<TokenStream>().unwrap().into_iter().collect::<Vec<_>>();
        assert!(residue(&input).is_some());
        let marked = mark(source);
        assert_eq!(marked, expected.parse::<TokenStream>().unwrap().to_string());
        assert!(
            residue(&marked.parse::<TokenStream>().unwrap().into_iter().collect::<Vec<_>>())
                .is_none()
        );
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

    #[test]
    fn complete_rust_patterns_roundtrip_inside_one_segment() {
        use quote::ToTokens;

        for pattern in [
            "A<B, C>",
            "(T, Vec<T>)",
            "&'a mut T",
            "*const T",
            "[T; N]",
            "[T]",
            "fn(T) -> U",
            "for<'a> unsafe extern \"C\" fn(&'a T) -> U",
            "<T as Trait<U>>::Assoc<V>",
            "dyn Trait<T> + Send",
        ] {
            let input = format!("({pattern}@..)").parse::<TokenStream>().unwrap();
            let output = mark_template(&input.into_iter().collect::<Vec<_>>(), 0).unwrap();
            let ty = syn::parse2::<syn::Type>(output.into_iter().collect()).unwrap();
            let syn::Type::Tuple(tuple) = ty else { panic!("not a tuple: {pattern}") };
            assert_eq!(tuple.elems.len(), 1, "{pattern}");
            let marker = tuple.elems.first().unwrap();
            assert!(is_varseg_type(marker), "{pattern}");
            assert_eq!(
                varseg_pattern(marker).unwrap().to_token_stream().to_string(),
                syn::parse_str::<syn::Type>(pattern).unwrap().to_token_stream().to_string(),
                "{pattern}"
            );
            assert_eq!(varseg_prefix(marker), None, "{pattern}");
        }
    }

    #[test]
    fn generic_slots_preserve_outer_patterns_and_inner_commas() {
        let source = "impl{Wrap<A<B, C>@.., &T@.., fn(T) -> U@.., Last>}";
        let expected = "impl{Wrap<[A<B, C>; ()], [&T; ()], [fn(T) -> U; ()], Last>}";
        assert_eq!(mark(source), expected.parse::<TokenStream>().unwrap().to_string());
    }

    #[test]
    fn named_function_arguments_and_bindings_keep_their_prefixes() {
        for (source, expected) in [
            (
                "impl{fn(#[opaque(A@..)] field: &T@.., plain: U)}",
                "impl{fn(#[opaque(A@..)] field: [&T; ()], plain: U)}",
            ),
            ("impl{Trait<Item = A<B>@..>}", "impl{Trait<Item = [A<B>; ()]>}"),
            ("impl{fn(#[opaque(A@..)] &T@..)}", "impl{fn(#[opaque(A@..)] [&T; ()],)}"),
        ] {
            assert_eq!(mark(source), expected.parse::<TokenStream>().unwrap().to_string());
        }
    }

    #[test]
    fn root_and_single_type_slots_do_not_gain_tuple_commas() {
        for (source, expected) in [
            ("impl{A<B>@..}", "impl{[A<B>; ()]}"),
            ("impl{[T@..; N]}", "impl{[[T; ()]; N]}"),
            ("impl{[T@..]}", "impl{[[T; ()]]}"),
            ("impl{Vec<T@..>@..}", "impl{[Vec<[T; ()]>; ()]}"),
        ] {
            assert_eq!(mark(source), expected.parse::<TokenStream>().unwrap().to_string());
        }
    }

    #[test]
    fn complex_segments_preserve_opaque_inputs_and_array_lengths() {
        for source in [
            "impl{[u8; (|[n @ ..]| n.len())([])]}",
            "opaque!(impl{(A<B>@..)})",
            "#[opaque(impl{(A<B>@..)})]",
            "impl{opaque!((T, Vec<T>)@..)}",
            "impl{Arr<{ match 2 { n @ ..=3 => n, _ => 0 } }>} ",
        ] {
            let input = source.parse::<TokenStream>().unwrap();
            assert_eq!(mark(source), input.to_string(), "{source}");
        }
        let source = "impl{(opaque!(A@..), Arr<{n @ ..}>)@..}";
        let expected = "impl{[(opaque!(A@..), Arr<{n @ ..}>); ()]}";
        assert_eq!(mark(source), expected.parse::<TokenStream>().unwrap().to_string());
    }

    #[test]
    fn malformed_slot_boundaries_never_leave_a_recognized_segment() {
        for prefix in ["", "<", ">", "&", "fn() ->", "T::", "Item =", "name:", "#[x]"] {
            for middle in ["A", "A<B>", "(A, B)", "[A; N]", "&A"] {
                for tail in ["", ", B@..", "> A@..", "@..", "<B@..>"] {
                    let source = format!("{prefix} {middle}@..{tail}");
                    let input = source.parse::<TokenStream>().unwrap();
                    let output = mark_template(&input.into_iter().collect::<Vec<_>>(), 0)
                        .unwrap_or_else(|error| panic!("{source}: {error}"));
                    assert!(first_unmarked_segment(&output).is_none(), "{source}");
                }
            }
        }
    }
}
