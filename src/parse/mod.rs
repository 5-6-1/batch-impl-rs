//! Parsing layer: DSL precedence-climbing parser and angle-bracket generic parsing.
//!
//! # Precedence hierarchy and its invariants
//!
//! Low → high: `;` < `,` < **space** (left-assoc, the Space level) < `.`
//! (right-assoc, the Dot level) < **blocks** (Prim). The chain layers cut
//! between **blocks** ([`crate::parse::space::parse_block`]): a block is the
//! smallest self-contained type fragment, and the space (which is not a
//! token) is recognized by the adjacency of two block starts. List levels
//! (`;` / `,`) cut by stop characters (`parse_operand` → `take_segment`).
//!
//! The invariants that keep the layers sound:
//!
//! 1. Blocks never swallow the type they would apply to (`&mut u8` is the
//!    two blocks `&mut` + `u8`) — except for lifetime references (`&'a mut u8`),
//!    the fn family (`fn(u8) -> u8`), and `*T` consuming one following block.
//! 2. All semantic combination happens in **apply** — the parse layer only
//!    cuts blocks. `<>` is a `TyTypeParam` block; whether it is a generic
//!    declaration or a trait/type argument is decided by the apply
//!    combinators (`TyTypeParam` as a left operand declares; a right operand
//!    with bounds/const declares; a plain-type right operand extends).
//! 3. Attachments (`{...}` / `where{...}` / `impl{...}`) are blocks too, so
//!    their position in the chain is irrelevant — the wrapper apply is
//!    "combine the inner, then re-wrap".

mod blocks;
mod chain;
mod generic;
mod ident_blocks;
mod parse_atom;
mod reentry;
mod space;
pub(crate) use chain::parse_item;
pub(crate) use generic::split_at_depth0;
// The one `<>`-group discriminator: a depth-0 `as` inside a leading angle group
// means a **qualified-self head** (`<T as Tr>`), everywhere that question is
// asked (this file's block parser, the ident-path parser, and the impl entry's
// `new-generic-decl` split).
pub(crate) use ident_blocks::split_projection;
pub(crate) use space::*;

use proc_macro2::{Group, Ident, TokenTree};

use crate::ast::*;
use crate::util::Cursor;

/// The parse layer's one piece of ambient state, carried down the whole
/// type-recursion by value (it is `Copy`).
///
/// * `trait_name` — the trait being implemented. A bare ident head that names
///   it is the **trait head** (`Trait U8` → `TyTrait`, so the args may carry
///   bindings); any other head is a plain type. This is the classification
///   authority for `TyTrait` vs `TyGeneric`.
/// * `bound` — the position is a **bound** one. Rust allows associated-type
///   bindings on *any* trait path in a bound (`T: Iterator<Item = u8>`), after
///   `dyn`, and after a `for<'a>` binder, while a plain type's args are a plain
///   type list (`Vec<Item = u8>` is a usage error). Set by the bound parsers,
///   and cleared again inside a nested args list — those chunks are types, not
///   bounds.
#[derive(Clone, Copy, Default)]
pub(crate) struct Ctx<'a> {
    pub(crate) trait_name: Option<&'a Ident>,
    pub(crate) bound: bool,
    /// Recursive block depth, including flat prefixes that introduce no group.
    block_depth: usize,
}

impl<'a> Ctx<'a> {
    /// A plain type position with the annotated trait's name.
    pub(crate) fn new(trait_name: Option<&'a Ident>) -> Self {
        Self { trait_name, ..Default::default() }
    }

    /// The same context in a **bound** position (see the type-level docs).
    pub(crate) fn in_bound(self) -> Self {
        Self { bound: true, ..self }
    }

    /// The same context in a **plain** type position: the bound flag never
    /// leaks into a nested type — an args-list chunk, a container element, a fn
    /// parameter or return type, a `&'a` / `?` / `!` target. Every one of those
    /// is a **sub-type position** of the bound element, not the element itself;
    /// a `dyn` / `for<'a>` / `impl` *inside* one re-enters a bound on its own.
    pub(crate) fn plain(self) -> Self {
        Self { bound: false, ..self }
    }
}

/// Resolves `@N` / `@g_i` position references inside a token chunk that is
/// **not** parsed as a type (angle-group contents go through flat token
/// splitting in `parse_type_params`, so `Box<@0>` would otherwise keep the
/// raw `@0`). Recurses into groups; every reference folds into the
/// self-delimiting carrier form (`@` + Brace group) that the parse layer and
/// the codegen resolvers both recognize; `@` followed by a non-digit errors.
///
/// The error is an [`AtRefError`] — the *message*, not a rendered stream — so
/// the consumer can pick the channel its position needs: a type position takes
/// `into_ty()` (an error node the entry aggregates, so no half-built impl is
/// emitted) and the recursion here takes `into_stream()`.
pub(crate) fn resolve_at_refs(
    tokens: &[TokenTree],
) -> Result<Vec<TokenTree>, crate::ast::fresh_protocol::AtRefError> {
    let mut out = Vec::with_capacity(tokens.len());
    let mut i = 0;
    while let Some(cur) = tokens.get(i) {
        match cur {
            TokenTree::Punct(p) if p.as_char() == '@' => {
                let at_span = p.span();
                match tokens.get(i + 1) {
                    // Already a carrier (`@{...}`): pass both tokens through
                    // untouched — the self-delimiting group is atomic and the
                    // resolvers downstream match this exact shape.
                    Some(TokenTree::Group(g)) if g.delimiter() == delimiter![{}] => {
                        out.push(cur.clone());
                        out.push(TokenTree::Group(g.clone()));
                        i += 2;
                    }
                    Some(TokenTree::Literal(lit)) => {
                        let lit_str = lit.to_string();
                        // `@N..` open range / `@N..M` / `@N..=M` closed range, or
                        // the grouped forms `@L_N..` / `@L_N..M` / `@L_N..=M`
                        // (within generator group L — stable across array
                        // dispatch) → the structured carrier `@{...}`. The
                        // Brace group is an atomic unit, so a range may appear
                        // anywhere a single `@N` can (`Wrapper<@0..>`,
                        // `<@0.. as T>::Scalar`).
                        let range_lit = parse_range_literal(&lit_str);
                        if let Some((group, start)) = range_lit
                            && let Some((op, _)) = crate::util::read_op(tokens, i + 2)
                            && matches!(op, crate::util::Op::DotDot | crate::util::Op::DotDotEq)
                        {
                            let inclusive = matches!(op, crate::util::Op::DotDotEq);
                            // The guard above admits only `..` / `..=`; the
                            // width is read without an `unreachable!` — a
                            // panic inside a proc macro is a compiler ICE.
                            let mut consumed = 2 + if inclusive { 3 } else { 2 };
                            // closed `@N..M` / `@N..=M`: an end literal.
                            // `@N..M` (exclusive) normalizes to the inclusive
                            // protocol (`..=M-1`), matching the where-predicate
                            // resolution (`FreshRef::Closed` is inclusive).
                            let end = match tokens.get(i + consumed) {
                                Some(TokenTree::Literal(el)) => {
                                    let Some(e) = el.to_string().parse::<usize>().ok() else {
                                        return Err(AtRefError::range_end_not_a_number(at_span));
                                    };
                                    consumed += 1;
                                    if inclusive || start < e {
                                        FreshEnd::Closed(if inclusive { e } else { e - 1 })
                                    } else {
                                        // empty exclusive range (`@2..1`)
                                        return Err(AtRefError::empty_exclusive_range(
                                            start, e, at_span,
                                        ));
                                    }
                                }
                                _ => FreshEnd::Open,
                            };
                            let r = FreshRef { group, start, end };
                            out.extend(fresh_ref_tokens(r, at_span));
                            i += consumed;
                            continue;
                        }
                        let r = parse_single_ref_token(&lit_str)
                            .ok_or_else(|| AtRefError::position_digit(at_span))?;
                        out.extend(fresh_ref_tokens(r, at_span));
                        i += 2;
                    }
                    _ => {
                        return Err(AtRefError::not_a_position_digit(at_span));
                    }
                }
            }
            TokenTree::Group(g) => {
                let inner = g.stream().into_iter().collect::<Vec<_>>();
                // The recursion keeps this function's own error type; only the
                // outermost caller chooses which channel renders it.
                let folded = resolve_at_refs(&inner)?;
                let mut new_g = Group::new(g.delimiter(), folded.into_iter().collect());
                new_g.set_span(g.span());
                out.push(TokenTree::Group(new_g));
                i += 1;
            }
            _ => {
                out.push(cur.clone());
                i += 1;
            }
        }
    }
    Ok(out)
}

/// Parses the literal after `@` in a range reference: `N` (flat) or `L_N`
/// (grouped, like `@g_i`). Returns `(group, start)` — `group: None` for the
/// flat form. Only digits-with-optional-underscore shapes qualify; anything
/// else (a bare digit is handled by the single-`@N` path) returns `None`.
pub(crate) fn parse_range_literal(s: &str) -> Option<(Option<usize>, usize)> {
    if let Ok(n) = s.parse::<usize>() {
        return Some((None, n));
    }
    let (l, n) = s.split_once('_')?;
    Some((Some(l.parse::<usize>().ok()?), n.parse::<usize>().ok()?))
}

/// Parses a single-position reference literal (`N` / `g_i`) into its
/// structured form — the token-chunk counterpart of `parse::blocks`'s
/// `parse_single_ref`.
fn parse_single_ref_token(lit: &str) -> Option<FreshRef> {
    use crate::ast::fresh_protocol::{FreshEnd, FreshRef};
    if let Ok(n) = lit.parse::<usize>() {
        return Some(FreshRef { group: None, start: n, end: FreshEnd::Single });
    }
    let (l, i) = lit.split_once('_')?;
    Some(FreshRef { group: Some(l.parse().ok()?), start: i.parse().ok()?, end: FreshEnd::Single })
}

/// The Prim level parses one block — the floor of the precedence ladder
/// (`Op::next()` from `Dot` yields `Prim`). The production chain reaches
/// blocks through `parse_block` at the Space/Dot levels, so this rung is
/// selected only by a direct `parse_item(Prim)` call. A token that cannot open
/// a block falls through to the primitive validation (`-` retirement, stray
/// `;`/`=`/`@`/`#`, ...).
pub(crate) fn parse_primitive(tokens: &[TokenTree], ctx: Ctx<'_>) -> Ty {
    let mut cursor = Cursor::new(tokens);
    parse_block(&mut cursor, ctx).unwrap_or_else(|| generic::primitive(tokens))
}

#[cfg(test)]
mod tests {
    use super::Ctx;
    // Verify the fn-family / trait-object block branches parse without error.
    // These run the real parse pipeline (angle_collect -> parse_item).
    fn parse_ok(s: &str) {
        let ts: proc_macro2::TokenStream = s.parse().unwrap();
        let v = crate::preprocess::angle_collect(&ts.into_iter().collect::<Vec<_>>()).unwrap();
        let mut c = crate::util::Cursor::new(&v);
        let ty = super::parse_item(&mut c, crate::ast::Op::Comma, Ctx::default());
        assert!(ty.is_some(), "parse failed for: {s}");
    }

    /// Parses `s` and renders it back (angle groups restored), panicking with the
    /// rendered text when the parse produced a diagnostic — an error `Ty` renders
    /// as `compile_error!(…)`, which is how the qualified-type gap used to show up.
    fn round_trip(s: &str) -> String {
        use quote::ToTokens as _;
        let ts: proc_macro2::TokenStream = s.parse().unwrap();
        let v = crate::preprocess::angle_collect(&ts.into_iter().collect::<Vec<_>>()).unwrap();
        let mut c = crate::util::Cursor::new(&v);
        let ty = super::parse_item(&mut c, crate::ast::Op::Comma, Ctx::default())
            .unwrap_or_else(|| panic!("no parse for `{s}`"));
        crate::preprocess::render_angles(ty.to_token_stream()).to_string()
    }

    /// Whitespace-insensitive comparison: proc-macro2's `to_string` spacing is an
    /// artifact, the token sequence is the contract.
    fn flat(s: &str) -> String {
        s.chars().filter(|c| !c.is_whitespace()).collect()
    }

    /// Qualified types now parse and round-trip: `Foo<T>::Assoc`,
    /// `Foo::<u8>::Assoc` and `<T as Tr>::Assoc`, in the target position and
    /// nested inside generics / further qualifications. Every one of these used to
    /// render `unexpected \`:\` after the type` — `angle_collect` had already
    /// paired the `<...>` into a group, so the `::`-tail had nowhere to attach.
    #[test]
    fn qualified_types_round_trip() {
        for s in [
            "Foo<T>::Assoc",
            "Foo::<u8>::Assoc",
            "<T as Tr>::Assoc",
            "Vec<<T as Tr>::Assoc>",
            "Box<Foo<T>::Assoc>",
            "<<T as Tr>::Assoc as Tr>::Out",
            "<T as Tr>::Assoc::More",
            "<T as Tr>::Item<u8>",
            "<T as Vec<u8>>::IntoIter",
            "std::vec::Vec<u8>::IntoIter",
        ] {
            let rendered = round_trip(s);
            assert!(!rendered.contains("compile_error"), "`{s}` must parse, got: {rendered}");
        }
    }

    /// The path spellings that already worked keep their exact token sequence —
    /// the new acceptance is additive, and `Foo::<u8>::Assoc` normalizes to the
    /// angle form (the same type in type position; the turbofish spelling was a
    /// parse error before, so nothing existing changes).
    #[test]
    fn existing_path_spellings_are_unchanged() {
        for (input, expected) in [
            ("Foo::Assoc", "Foo::Assoc"),
            ("Self::Item", "Self::Item"),
            ("Vec<Foo::Assoc>", "Vec<Foo::Assoc>"),
            ("std::vec::Vec<u8>", "std::vec::Vec<u8>"),
            ("Foo<T>::Assoc", "Foo<T>::Assoc"),
            ("Foo::<u8>::Assoc", "Foo<u8>::Assoc"),
            ("<T as Tr>::Assoc", "<T as Tr>::Assoc"),
        ] {
            assert_eq!(flat(&round_trip(input)), flat(expected), "input `{input}`");
        }
    }

    /// A `::`-tail is plain Rust path text, so DSL tokens inside it are reported
    /// rather than leaked (`Assoc<@0>` would reach rustc as "expected type, found
    /// `@`"), and a bare `::@0` becomes an apply operand — which a qualified type
    /// rejects with its own diagnostic. A tail that simply **ends** at the `::`
    /// gets the missing-segment wording instead of the DSL-token one (second
    /// review, N4 — the old message described the token case only).
    #[test]
    fn qualified_tail_rejects_dsl_tokens() {
        let rendered = round_trip("Foo<T>::Assoc<@0>");
        assert!(rendered.contains("plain Rust path"), "got: {rendered}");
        let rendered = round_trip("<T as Tr>::@0");
        assert!(rendered.contains("`::`-tail segment must be an identifier"), "got: {rendered}");
        let rendered = round_trip("Foo<T>::");
        assert!(rendered.contains("`::` must be followed by a path segment"), "got: {rendered}");
    }

    /// A single-element matrix is written **bare** (`<T as Tr>::Assoc : <u8 as Tr>::Assoc`):
    /// `[T]` in a matrix source is an array/slice *type* (one leaf), not a list —
    /// the list form needs a comma (`[A, B]`). Locked here because the qualified
    /// template test depends on it.
    #[test]
    fn matrix_single_element_is_bare() {
        let ts: proc_macro2::TokenStream = "<u8 as Tr>::Assoc".parse().unwrap();
        let v = crate::preprocess::angle_collect(&ts.into_iter().collect::<Vec<_>>()).unwrap();
        let mut c = crate::util::Cursor::new(&v);
        let ty = super::parse_item(&mut c, crate::ast::Op::Comma, Ctx::default());
        assert!(ty.is_some(), "a bare qualified type parses as one leaf");
        let ts: proc_macro2::TokenStream = "[<u8 as Tr>::Assoc]".parse().unwrap();
        let v = crate::preprocess::angle_collect(&ts.into_iter().collect::<Vec<_>>()).unwrap();
        let mut c = crate::util::Cursor::new(&v);
        let ty = super::parse_item(&mut c, crate::ast::Op::Comma, Ctx::default());
        assert!(ty.is_some(), "`[T]` parses too — as the array type, one leaf");
    }

    /// Associated-type bindings are legal on **any** trait path in a bound
    /// position: a generic declaration's bound (`<T: Iterator<Item = u8>>`), a
    /// trait object (`dyn Iterator<Item = u8>`), a `for<'a>` binder, and
    /// wherever those nest (`Box<dyn …>`). A plain type's args stay a plain
    /// type list — `Vec<Item = u8>` keeps its targeted diagnostic (locked by
    /// `tests/ui/concrete_binding.rs`).
    #[test]
    fn bindings_are_accepted_in_bound_positions() {
        for s in [
            "<T: Iterator<Item = u8>>",
            "<T: Iterator<Item = u8> + Clone>",
            "dyn Iterator<Item = u8>",
            "Box<dyn Iterator<Item = u8>>",
            "dyn (Iterator<Item = u8>)",
            "for<'a> Iterator<Item = u8>",
            "dyn Iterator<Item = <u8 as Tr>::Assoc>",
        ] {
            let rendered = round_trip(s);
            assert!(!rendered.contains("compile_error"), "`{s}` must parse, got: {rendered}");
            assert_eq!(flat(&rendered), flat(s), "`{s}` must round-trip unchanged");
        }
        // The flag never leaks into a nested args list: those chunks are types.
        for s in ["Vec<Item = u8>", "Box<Iterator<Item = u8>>", "<T: Into<Vec<Item = u8>>>"] {
            let rendered = round_trip(s);
            assert!(
                rendered.contains("binding args"),
                "`{s}` is a plain arg list — expected the targeted error, got: {rendered}"
            );
        }
        // …nor into a **sub-type position** of a bound element — only the head
        // of the element itself may carry bindings. (Every one of these is
        // invalid Rust anyway; what is locked is that the DSL keeps its own
        // targeted diagnostic instead of emitting the tokens for rustc.)
        //
        // A comma-less `(...)` is deliberately *not* in this list: Rust reads it
        // as a parenthesized bound in a bound position (`T: (Iterator<Item = u8>)`
        // and `dyn (Iterator<Item = u8>)` both compile), so the flag passes
        // through and `dyn (Iterator<Item = u8>)` stays accepted.
        //
        // The last case is the space fold's **follower** (`*const` is the head,
        // `Vec<Item = u8>` its apply operand): a space-applied bound is not even
        // valid syntax in Rust (`T: Clone Vec<Item = u8>` is a parse error at
        // `Vec`), so nothing legal is affected by clearing the flag there.
        for s in [
            "<T: fn(Vec<Item = u8>)>",
            "<T: fn() -> Vec<Item = u8>>",
            "<T: (Vec<Item = u8>,)>",
            "<T: [Vec<Item = u8>; 1]>",
            "<T: &'static Vec<Item = u8>>",
            "<T: ?Vec<Item = u8>>",
            "<T: *const Vec<Item = u8>>",
        ] {
            let rendered = round_trip(s);
            assert!(
                rendered.contains("binding args"),
                "`{s}` — a sub-type position of a bound must keep the error, got: {rendered}"
            );
        }
    }

    /// A leading `::` makes a path **global** (`::std::vec::Vec<u8>`). The `::`
    /// belongs to the block, so it works at the start of a spec, nested in an
    /// args list, before a `::`-tail, and as the projection head's type
    /// (`<::std::vec::Vec<u8> as IntoIterator>::IntoIter`) — and it must be
    /// followed by a path segment identifier.
    #[test]
    fn global_paths_parse() {
        for s in [
            "::std::vec::Vec<u8>",
            "::std::vec::Vec",
            "Box<::std::vec::Vec<u8>>",
            "::core::option::Option<u8>",
            // a `::`-tail after a global head (rustc needs the fully-qualified
            // form for this *type*, but the parser's job is token fidelity)
            "::std::vec::Vec<u8>::IntoIter",
            "<::std::vec::Vec<u8> as IntoIterator>::IntoIter",
        ] {
            let rendered = round_trip(s);
            assert!(!rendered.contains("compile_error"), "`{s}` must parse, got: {rendered}");
            assert_eq!(flat(&rendered), flat(s), "`{s}` must round-trip unchanged");
        }
        let rendered = round_trip("::@0");
        assert!(
            rendered.contains("must be followed by a path segment identifier"),
            "got: {rendered}"
        );
    }

    /// A `fn(...)` **pointer** type accepts named parameters (`fn(x: u8)` is
    /// valid Rust). The `Fn(...)` sugar does not — rustc's own rule for
    /// `Trait(...)` syntax — so it gets a targeted message, and a name without
    /// a type is reported instead of rendered as `x:`.
    #[test]
    fn named_fn_parameters_parse() {
        for s in ["fn(x: u8) -> u8", "fn(u8, y: u8)", "fn(_: u8)", "fn(x: Box<u8>) -> u8"] {
            let rendered = round_trip(s);
            assert!(!rendered.contains("compile_error"), "`{s}` must parse, got: {rendered}");
            assert_eq!(flat(&rendered), flat(s), "`{s}` must round-trip unchanged");
        }
        // A trailing comma is dropped by the fn renderer (`fn(u8,)` → `fn(u8)`).
        assert_eq!(flat(&round_trip("fn(x: u8,)")), flat("fn(x: u8)"));
        // The ABI prefix preserves the structured fn's named parameter.
        assert_eq!(flat(&round_trip("extern \"C\" fn(x: u8)")), flat("extern \"C\" fn(x: u8)"));
        let rendered = round_trip("dyn Fn(x: u8) -> u8");
        assert!(rendered.contains("does not support named parameters"), "got: {rendered}");
        let rendered = round_trip("fn(x:) -> u8");
        assert!(rendered.contains("missing a type"), "got: {rendered}");
    }

    /// Regression (the third fuzz-hang root cause, found by adversarial review):
    /// The former token-only extern return parser lacked a progress check.
    /// ABI fn pointers now share the structural return parser; that live path
    /// must retain the guard rather than keeping a test-only legacy helper.
    /// `#` followed by a non-bracket group is `starts_block`-true and
    /// `parse_block`-none, so the loop spun on an unmoved cursor — allocating
    /// nothing, so the fuzz `GuardAlloc` could not catch it; the compiler just
    /// hung. It must terminate with the targeted diagnostic and consume the
    /// stalled token.
    #[test]
    fn return_tokens_stall_terminates_with_diagnostic() {
        let ts: proc_macro2::TokenStream = "u8 # (x)".parse().unwrap();
        let v = ts.into_iter().collect::<Vec<_>>();
        let mut c = crate::util::Cursor::new(&v);
        let outcome = super::space::parse_return_expr(&mut c, super::Ctx::default());
        use quote::ToTokens as _;
        let rendered = crate::preprocess::render_angles(outcome.to_token_stream()).to_string();
        assert!(rendered.contains("unexpected `#`"), "expected the stalled token, got: {rendered}");
        // Progress: the stalled `#` is consumed, so the fold cannot spin on it.
        // (What follows it is left to the chain, which folds it into the error.)
        assert!(
            !matches!(c.peek(), Some(t) if t.to_string() == "#"),
            "the stalled `#` must be consumed"
        );
    }

    #[test]
    fn fn_mut_parses() {
        parse_ok("dyn FnMut(u8) -> u8");
    }

    /// Regression (the second fuzz-OOM root cause): a lone `'` accepted by
    /// `starts_block` but rejected unconsumed by `parse_block` used to spin
    /// the space/bound fold loops forever, appending one empty arg per
    /// iteration until memory died. It must terminate with the targeted
    /// diagnostic — in a bound, and anywhere else the fold loops run.
    /// Built from raw token trees: proc-macro2's lexer rejects a lone `'`
    /// before our parser ever sees it (exactly how fuzz reaches it).
    #[test]
    fn lone_quote_terminates_with_diagnostic() {
        use proc_macro2::{Group, Ident, TokenTree};
        fn p(c: char) -> TokenTree {
            TokenTree::Punct(proc_macro2::Punct::new(c, proc_macro2::Spacing::Alone))
        }
        fn id(s: &str) -> TokenTree {
            TokenTree::Ident(Ident::new(s, proc_macro2::Span::call_site()))
        }
        // bound context: a None group parsed as generic args (`T: ' &`)
        let bound = vec![
            TokenTree::Group(Group::new(
                delimiter![none],
                [id("T"), p(':'), p('\''), p('&')].into_iter().collect(),
            )),
            id("usize"),
        ];
        // plain space chain: `usize ' &`
        let chain = vec![id("usize"), p('\''), p('&')];
        // bound tail: `T: Clone '`
        let tail = vec![id("T"), p(':'), id("Clone"), p('\'')];
        // (expected diagnostic fragment, tokens) — a top-level bare `T:`
        // hits the generic boundary diagnostic; what matters is that every
        // fold terminates
        for (expect, toks) in [("lone `'`", bound), ("lone `'`", chain), ("unexpected `:`", tail)] {
            let mut c = crate::util::Cursor::new(&toks);
            let mut out = String::new();
            use quote::ToTokens as _;
            while !c.at_end() {
                let Some(ty) =
                    crate::parse::parse_item(&mut c, crate::ast::Op::Comma, Ctx::default())
                else {
                    break;
                };
                // The fold loops must make progress: an unbounded run here
                // hangs the test (which is the regression being locked).
                out.push_str(&crate::preprocess::render_angles(ty.to_token_stream()).to_string());
            }
            assert!(out.contains(expect), "expected `{expect}`, got: {out}");
        }
    }

    /// Regression (same family as the fuzz OOM): a huge literal endpoint
    /// must reject arithmetically, never reserve a range-sized Vec first.
    #[test]
    fn huge_range_endpoint_rejects_without_allocating() {
        let ts: proc_macro2::TokenStream = "T.0..4000000000".parse().unwrap();
        let v = crate::preprocess::angle_collect(&ts.into_iter().collect::<Vec<_>>()).unwrap();
        let mut c = crate::util::Cursor::new(&v);
        let ty = crate::parse::parse_item(&mut c, crate::ast::Op::Comma, Ctx::default()).unwrap();
        use quote::ToTokens as _;
        let out = crate::preprocess::render_angles(ty.to_token_stream()).to_string();
        assert!(out.contains("limit 1024"), "expected the range limit, got: {out}");
    }

    /// Regression (the fuzz OOM root cause): a composed array×range chain
    /// multiplies leaves per nesting level with no intermediate check — it
    /// must hit the expansion limit as a diagnostic, never balloon memory.
    #[test]
    fn composed_range_chain_hits_limit() {
        let spec = "((((([T,T].0..3).0..3).0..3).0..3).0..3).0..3";
        let ts: proc_macro2::TokenStream = spec.parse().unwrap();
        let v = crate::preprocess::angle_collect(&ts.into_iter().collect::<Vec<_>>()).unwrap();
        let mut c = crate::util::Cursor::new(&v);
        let ty = crate::parse::parse_item(&mut c, crate::ast::Op::Comma, Ctx::default()).unwrap();
        use quote::ToTokens as _;
        let out = crate::preprocess::render_angles(ty.to_token_stream()).to_string();
        assert!(out.contains("limit 1024"), "expected the range-chain expansion limit, got: {out}");
    }

    #[test]
    fn fn_once_parses() {
        parse_ok("dyn FnOnce(u8) -> u8");
    }

    #[test]
    fn impl_trait_parses() {
        parse_ok("impl Fn(u8) -> u8");
        parse_ok("impl Iterator + Clone");
    }

    #[test]
    fn for_hrtb_parses() {
        parse_ok("for<'a> fn(&'a u8) -> &'a u8");
    }

    #[test]
    fn prefix_puncts_parse() {
        // `?` / `!` prefix puncts are structured prefix blocks; `self` is the
        // identity prefix (`self.T` => `T`).
        parse_ok("?Sized");
        parse_ok("! u8");
        parse_ok("self u8");
        parse_ok("self.Box u8");
    }
}
