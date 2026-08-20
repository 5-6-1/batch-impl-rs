//! Space-application parsing: `Box u8` / `HashMap u32 String` — adjacent
//! types separated by a space are a left-associative application (the
//! successor of the `-` operator). The space is not a token, so a "space
//! unit" scanner cuts the token stream at adjacency boundaries (an atom
//! directly followed by another atom) and at explicit operators/attachments.
//!
//! Precedence (low → high): space (left-assoc) < `.` (right-assoc) < atom.
//! A space unit is one full type — its inner `.` chain, generics, `::` paths,
//! prefixes, `..` ranges and `@` refs stay whole — and the chain folds left.

use proc_macro2::{Delimiter, Spacing, TokenTree};

/// Whether an ident opens a prefix-taking type (`dyn Trait`, `impl Trait`,
/// `fn(...)`, `unsafe fn`, `self`, `const`, `&mut`, `async fn`): the ident
/// and the type it qualifies are one unit (no space-application boundary
/// inside).
fn is_prefix_ident(s: &str) -> bool {
    matches!(s, "dyn" | "impl" | "fn" | "unsafe" | "self" | "const" | "mut" | "async")
}

/// Whether `tokens[i]` starts a new space-application unit: an atom, a
/// prefix punct (`&`/`*`/`?`/`!`/`@`) or an attribute (`#[...]`). A Brace
/// group is an attachment, not a unit (`{body}` / `where{...}` / `impl{...}`
/// are stripped before the chain runs); `#` alone and `-` (retired infix)
/// are not unit starts.
pub(crate) fn starts_unit(t: &TokenTree) -> bool {
    matches!(t, TokenTree::Ident(_) | TokenTree::Literal(_))
        || matches!(t, TokenTree::Group(g) if g.delimiter() != Delimiter::Brace)
        || matches!(t, TokenTree::Punct(p) if matches!(p.as_char(), '&' | '*' | '?' | '!' | '@'))
}

/// Whether `tokens[i]` is the first `.` of a `..` range (a Joint `.` whose
/// next token is another `.`).
fn is_dotdot(tokens: &[TokenTree], i: usize) -> bool {
    matches!(tokens.get(i), Some(TokenTree::Punct(p))
        if p.as_char() == '.'
            && matches!(tokens.get(i + 1), Some(TokenTree::Punct(q)) if q.as_char() == '.'))
}

/// Whether `tokens[i]` is the `-` of a `->` fn arrow (Joint `-` followed by `>`).
fn is_arrow_dash(tokens: &[TokenTree], i: usize) -> bool {
    matches!(tokens.get(i), Some(TokenTree::Punct(p))
        if p.as_char() == '-'
            && p.spacing() == Spacing::Joint
            && matches!(tokens.get(i + 1), Some(TokenTree::Punct(q)) if q.as_char() == '>'))
}

/// Cuts the first **space unit** from `tokens` starting at `start`: one full
/// type up to the next adjacency boundary (an atom directly followed by an
/// atom — the space application), an explicit operator (`,` `;` `-` `#`), an
/// attachment (`{` / `where`), or the end. Returns `(unit_end, boundary)` —
/// `boundary` is `Some(i)` when another unit/operator follows at `i`, `None`
/// when the unit runs to the end of `tokens`.
pub(crate) fn scan_space_unit(tokens: &[TokenTree], start: usize) -> (usize, Option<usize>) {
    let end = scan_unit_atom(tokens, start);
    if end >= tokens.len() {
        return (tokens.len(), None);
    }
    (end, Some(end))
}

/// Consumes one atom (ident/literal/group, or a prefix + its qualified type)
/// plus its whole-unit suffixes (`::` paths, generic/group args, `.` apply
/// chains, `..` ranges, `->` fn arrows, `@` refs, trait-object `+`),
/// returning the index just past it (at the next boundary).
fn scan_unit_atom(tokens: &[TokenTree], mut i: usize) -> usize {
    if i >= tokens.len() {
        return i;
    }
    // Prefixes: the prefix + the type it qualifies are one unit. The
    // qualified type may be empty (bare `fn`, `&`, `dyn`, ...), so fall
    // through to the suffix loop instead of returning the recursion result —
    // `fn.(A,B)` must keep the `.`-apply inside the unit.
    match &tokens[i] {
        // `for<'a> Trait`: the bound-group and the qualified type join too
        // (`for<'a> fn(&'a u8) -> u8` is one HRTB unit) — unlike the other
        // prefixes, `for` always takes an angle group *then* a type.
        TokenTree::Ident(id) if id == "for" => {
            i = scan_unit_atom(tokens, scan_unit_atom(tokens, i + 1));
        }
        // `extern "C" fn(...)`: the ABI string literal joins the unit.
        TokenTree::Ident(id) if id == "extern" => {
            i += 1;
            if matches!(tokens.get(i), Some(TokenTree::Literal(_))) {
                i += 1;
            }
            i = scan_unit_atom(tokens, i);
        }
        TokenTree::Ident(id) if is_prefix_ident(&id.to_string()) => {
            i = scan_unit_atom(tokens, i + 1);
        }
        TokenTree::Punct(p) if matches!(p.as_char(), '&' | '*' | '?' | '!') => {
            i = scan_unit_atom(tokens, i + 1);
        }
        TokenTree::Punct(p) if p.as_char() == '@' => {
            i = scan_unit_atom(tokens, i + 2);
        }
        // `'a` lifetime: the quote + ident are one unit fragment, and the
        // type it qualifies joins too (`&'a T` is one unit, not `&'a` `T`).
        TokenTree::Punct(p)
            if p.as_char() == '\'' && matches!(tokens.get(i + 1), Some(TokenTree::Ident(_))) =>
        {
            i = scan_unit_atom(tokens, i + 2);
        }
        // `#[attr]`: the attribute is one unit prefix (attributes belong at
        // the spec start, before the chain can run). Skip the hash + group;
        // the suffix loop below keeps any `.`-apply after it in the unit.
        TokenTree::Punct(p)
            if p.as_char() == '#'
                && matches!(
                    tokens.get(i + 1),
                    Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Bracket
                ) =>
        {
            i += 2;
        }
        _ => {
            // a plain atom: ident / literal / non-Brace group
            if !(matches!(tokens.get(i), Some(TokenTree::Ident(_)) | Some(TokenTree::Literal(_)))
                || matches!(tokens.get(i), Some(TokenTree::Group(g)) if g.delimiter() != Delimiter::Brace))
            {
                return i; // not an atom start (operator / attachment boundary)
            }
            i += 1;
        }
    }
    loop {
        if i >= tokens.len() {
            return i;
        }
        match &tokens[i] {
            TokenTree::Punct(p) if p.as_char() == ':' && p.spacing() == Spacing::Joint => {
                i += 2; // `::`
                if matches!(tokens.get(i), Some(TokenTree::Ident(_))) {
                    i += 1; // the path segment after `::` stays in the unit
                }
            }
            // Generic / fn-call args stay in the unit only when attached to
            // an ident (`Vec<u8>`, `Fn(A)`); a bracket group after an ident
            // (`Pair2 [u8, u16]`) or any group after a group/literal
            // (`[] [A, B]`, `().3 ().3`) is a **separate space unit**, and a
            // Brace group is an attachment boundary (stripped before the
            // chain runs).
            TokenTree::Group(g)
                if g.delimiter() != Delimiter::Brace
                    && matches!(g.delimiter(), Delimiter::None | Delimiter::Parenthesis)
                    && matches!(tokens.get(i - 1), Some(TokenTree::Ident(_))) =>
            {
                i += 1;
            }
            // `ident![...]` / `ident!(...)` / `ident!{...}` — a macro call is
            // one unit (passthrough primitive), not `ident` applied to `!`.
            TokenTree::Punct(p)
                if p.as_char() == '!' && matches!(tokens.get(i + 1), Some(TokenTree::Group(_))) =>
            {
                i += 2;
            }
            TokenTree::Punct(p) if p.as_char() == '@' => {
                i = scan_unit_atom(tokens, i + 2); // `@N` position ref
            }
            TokenTree::Punct(p) if p.as_char() == '+' => {
                i = scan_unit_atom(tokens, i + 1); // trait-object bound type
            }
            // `fn(A) -> B` — the arrow and its return type stay in the unit;
            // the return type is a full space-expression (`-> Box u8` =
            // returning `Box<u8>`), so its units are consumed greedily.
            TokenTree::Punct(p) if is_arrow_dash(tokens, i) => {
                i += 2; // `->`
                loop {
                    let (end, boundary) = scan_space_unit(tokens, i);
                    i = end;
                    if !boundary.is_some_and(|at| starts_unit(&tokens[at])) {
                        break;
                    }
                }
            }
            TokenTree::Punct(p) if p.as_char() == '.' => {
                if is_dotdot(tokens, i) {
                    // `..` / `..=N` range — the whole range is one unit
                    i += 1; // first `.`
                    if matches!(tokens.get(i), Some(TokenTree::Punct(q)) if q.as_char() == '.') {
                        i += 1; // second `.`
                        if matches!(tokens.get(i), Some(TokenTree::Punct(q)) if q.as_char() == '=')
                        {
                            i += 1; // `..=`
                        }
                        i = scan_unit_atom(tokens, i); // range end point
                    }
                } else {
                    // `.` apply — the right operand joins the unit
                    i = scan_unit_atom(tokens, i + 1);
                }
            }
            _ => break,
        }
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::{Group, TokenStream};

    fn cut(s: &str) -> (usize, Option<usize>) {
        let v = s.parse::<TokenStream>().unwrap().into_iter().collect::<Vec<_>>();
        scan_space_unit(&v, 0)
    }

    /// `X<...>` in real DSL is angle-paired into a None group before the
    /// parse layer runs; the tests build the same shape by hand.
    fn paired(inner: &str) -> Group {
        let ts: TokenStream = inner.parse().unwrap();
        Group::new(delimiter![<>], ts)
    }

    #[test]
    fn single_ident_runs_to_end() {
        let (end, b) = cut("Box");
        assert_eq!(end, 1);
        assert_eq!(b, None);
    }

    #[test]
    fn adjacent_idents_split() {
        let (end, b) = cut("Box u8 u16");
        assert_eq!(end, 1, "Box is the first unit");
        assert_eq!(b, Some(1), "u8 is the boundary");
    }

    #[test]
    fn path_one_unit() {
        let (end, b) = cut("std::vec::Vec u16");
        // std + :: + vec + :: + Vec = 7 tokens (`::` is two Puncts)
        assert_eq!(end, 7, "std::vec::Vec is one unit");
        assert_eq!(b, Some(7));
    }

    #[test]
    fn generics_one_unit() {
        let v = vec![
            proc_macro2::TokenTree::Ident(proc_macro2::Ident::new(
                "Vec",
                proc_macro2::Span::call_site(),
            )),
            paired("u8").into(),
            "u16".parse::<TokenStream>().unwrap().into_iter().next().unwrap(),
        ];
        let (end, b) = scan_space_unit(&v, 0);
        assert_eq!(end, 2, "Vec<u8> (paired) is one unit");
        assert_eq!(b, Some(2));
    }

    #[test]
    fn dot_chain_one_unit() {
        let (end, b) = cut("Box.u8 u16");
        assert_eq!(end, 3, "Box.u8 is one unit (the . stays inside)");
        assert_eq!(b, Some(3));
    }

    #[test]
    fn range_one_unit() {
        let (end, b) = cut("1..=4 u8");
        assert_eq!(end, 5, "1..=4 is one unit (5 tokens)");
        assert_eq!(b, Some(5));
    }

    #[test]
    fn prefix_ident_one_unit() {
        let (end, b) = cut("dyn Trait u8");
        assert_eq!(end, 2, "dyn Trait is one unit");
        assert_eq!(b, Some(2));
    }

    #[test]
    fn trait_object_plus_one_unit() {
        let (end, b) = cut("dyn Trait + Send u8");
        assert_eq!(end, 4, "dyn Trait + Send is one unit");
        assert_eq!(b, Some(4));
    }

    #[test]
    fn tuple_group_one_unit() {
        let (end, b) = cut("(A, B) u8");
        assert_eq!(end, 1, "the tuple group is one unit");
        assert_eq!(b, Some(1));
    }

    #[test]
    fn operator_boundary() {
        let (end, b) = cut("Box, u8");
        assert_eq!(end, 1);
        assert_eq!(b, Some(1), "the comma is the boundary");
    }

    #[test]
    fn prefix_punct_one_unit() {
        let (end, b) = cut("&mut u8 u16");
        assert_eq!(end, 3, "&mut u8 is one unit");
        assert_eq!(b, Some(3));
    }

    #[test]
    fn at_ref_one_unit() {
        let v = vec![
            proc_macro2::TokenTree::Ident(proc_macro2::Ident::new(
                "Box",
                proc_macro2::Span::call_site(),
            )),
            paired("@0").into(),
            "u8".parse::<TokenStream>().unwrap().into_iter().next().unwrap(),
        ];
        let (end, b) = scan_space_unit(&v, 0);
        assert_eq!(end, 2, "Box<@0> (paired) is one unit");
        assert_eq!(b, Some(2));
    }

    #[test]
    fn bare_fn_dot_apply_one_unit() {
        let (end, b) = cut("fn.(A, B)");
        assert_eq!(end, 3, "bare fn + `.`-apply stays one unit");
        assert_eq!(b, None);
    }

    #[test]
    fn arrow_return_type_one_unit() {
        let (end, b) = cut("fn(u8) -> Box u8");
        assert_eq!(end, 6, "the arrow's return type is a full space-expression");
        assert_eq!(b, None);
    }

    #[test]
    fn brace_group_is_boundary() {
        let (end, b) = cut("Box { x }");
        assert_eq!(end, 1, "Box is the unit; the brace block is an attachment boundary");
        assert_eq!(b, Some(1));
    }

    #[test]
    fn attribute_one_unit() {
        let (end, b) = cut("#[allow(dead_code)] Box u8");
        assert_eq!(end, 2, "the attribute is one unit");
        assert_eq!(b, Some(2));
    }

    #[test]
    fn impl_trait_one_unit() {
        let (end, b) = cut("impl Trait u8");
        assert_eq!(end, 2, "impl Trait is one unit (impl-trait passthrough)");
        assert_eq!(b, Some(2));
    }

    #[test]
    fn minus_is_boundary() {
        let (end, b) = cut("Box - u8");
        assert_eq!(end, 1, "Box is the unit; `-` (retired infix) is a boundary");
        assert_eq!(b, Some(1));
    }
}
