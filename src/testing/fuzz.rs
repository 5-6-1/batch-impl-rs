//! Property-based testing (proptest) with a no-panic property.
//!
//! The library's promise is "no panic on user input". Feed random token sequences to the
//! dangerous entry points and assert that no input panics — `Err` / `None` / `compile_error!`
//! results are all accepted. Coverage: bare where rewrite, DSL parsing, and the **full
//! pipeline** (instruction preprocessing → where rewrite → parse/expand → generate impl,
//! incl. apply/expand/codegen).

use proc_macro2::{Delimiter, Group, Ident, Literal, Punct, Spacing, TokenStream, TokenTree};
use proptest::prelude::*;
use std::str::FromStr;

use crate::ast::Op;
use crate::entry::expand_attr_macro;
use crate::parse::parse_item;
use crate::preprocess::where_process;
use crate::util::Cursor;

/// Recursively generatable token description (Groups nest Vec<Tok>, depth-limited)
#[derive(Clone, Debug)]
enum Tok {
    Ident(&'static str),
    Literal(&'static str),
    Punct(char, Spacing),
    Group(Delimiter, Vec<Tok>),
}

/// Depth-limited token list generator (covers DSL keywords, operators, bracket nesting)
fn tokens(depth: usize) -> impl Strategy<Value = Vec<Tok>> {
    let leaf = prop_oneof![
        // DSL / Rust keywords and common type names
        Just(Tok::Ident("usize")),
        Just(Tok::Ident("isize")),
        Just(Tok::Ident("Vec")),
        Just(Tok::Ident("Box")),
        Just(Tok::Ident("T")),
        Just(Tok::Ident("where")),
        Just(Tok::Ident("fn")),
        Just(Tok::Ident("self")),
        Just(Tok::Ident("unsafe")),
        // Directive words: drive the `#` directive and open-extension paths
        // in the full-pipeline fuzz (the no-panic promise covers them too).
        Just(Tok::Ident("blanket")),
        Just(Tok::Ident("fill")),
        Just(Tok::Ident("delegate")),
        Just(Tok::Ident("name")),
        Just(Tok::Ident("all")),
        // Constant-system words: built-in families / range endpoints / the
        // `@trait` marker / blanket's `@Cow` — the `@` punct below can now
        // reach the constant expansion, range, and lifetime paths.
        Just(Tok::Ident("u8")),
        Just(Tok::Ident("i32")),
        Just(Tok::Ident("f64")),
        Just(Tok::Ident("Cow")),
        Just(Tok::Ident("trait")),
        // Numeric literals (small-integer DSL exponents)
        Just(Tok::Literal("0")),
        Just(Tok::Literal("1")),
        Just(Tok::Literal("3")),
        // DSL operators and punctuation
        Just(Tok::Punct('<', Spacing::Alone)),
        Just(Tok::Punct('>', Spacing::Alone)),
        Just(Tok::Punct('.', Spacing::Alone)),
        Just(Tok::Punct('-', Spacing::Alone)),
        Just(Tok::Punct(',', Spacing::Alone)),
        Just(Tok::Punct(';', Spacing::Alone)),
        Just(Tok::Punct(':', Spacing::Alone)),
        // A Joint `:` can combine with the next `:` into `::`
        Just(Tok::Punct(':', Spacing::Joint)),
        Just(Tok::Punct('&', Spacing::Alone)),
        Just(Tok::Punct('*', Spacing::Alone)),
        Just(Tok::Punct('#', Spacing::Alone)),
        Just(Tok::Punct('!', Spacing::Alone)),
        Just(Tok::Punct('=', Spacing::Alone)),
        // `@` constants, `..`/`..=` ranges (Joint `.` heads a range), `'`
        // lifetimes, and bound/bound-start punctuation — the paths the old
        // vocabulary could never reach.
        Just(Tok::Punct('@', Spacing::Alone)),
        Just(Tok::Punct('.', Spacing::Alone)),
        Just(Tok::Punct('.', Spacing::Joint)),
        Just(Tok::Punct('+', Spacing::Alone)),
        Just(Tok::Punct('?', Spacing::Alone)),
        Just(Tok::Punct('\'', Spacing::Alone)),
    ];
    if depth == 0 {
        prop::collection::vec(leaf, 0..6).boxed()
    } else {
        let grouped = prop_oneof![
            Just(delimiter![()]),
            Just(delimiter![[]]),
            Just(delimiter![{}]),
            // Real None groups simulate macro-variable expansion output — angle_collect
            // should flatten them (contents are DSL tokens)
            Just(delimiter![none]),
        ]
        .prop_flat_map(move |d| tokens(depth - 1).prop_map(move |inner| Tok::Group(d, inner)));
        prop::collection::vec(prop_oneof![leaf, grouped], 0..6).boxed()
    }
}

fn to_token(tok: &Tok) -> TokenTree {
    match tok {
        Tok::Ident(s) => Ident::new(s, proc_macro2::Span::call_site()).into(),
        Tok::Literal(s) => Literal::from_str(s).unwrap().into(),
        Tok::Punct(c, sp) => Punct::new(*c, *sp).into(),
        Tok::Group(d, inner) => {
            let stream = inner.iter().map(to_token).collect();
            Group::new(*d, stream).into()
        }
    }
}

/// A fixed proptest config for the fuzz suite: `cases` caps the per-test
/// random corpus, so a run is reproducible in time/memory on any machine.
/// The historical reduction to 64 worked around a multi-GB allocation whose
/// root cause (composed array×range chains multiplying leaves per nesting
/// level, invisible to the list-chain check) is fixed — every growth point
/// now enforces the expansion limit and the driver carries a global
/// per-spec backstop, so the default 256 is safe again.
fn fuzz_config() -> proptest::test_runner::Config {
    proptest::test_runner::Config { cases: 256, ..Default::default() }
}

proptest! {
    #![proptest_config(fuzz_config())]
    /// Bare where rewrite: no panic on arbitrary token input
    #[test]
    fn where_process_no_panic(toks in tokens(3)) {
        let ts = toks.iter().map(to_token).collect::<Vec<_>>();
        let _ = where_process(&ts);
    }

    /// DSL parsing: no panic on arbitrary token input, and it advances properly to the end
    #[test]
    fn parse_no_panic(toks in tokens(3)) {



        let ts = toks.iter().map(to_token).collect::<Vec<_>>();
        let mut cursor = Cursor::new(&ts);
        while parse_item(&mut cursor, Op::Comma, None).is_some() {}
        prop_assert!(cursor.at_end());
    }

    /// Full pipeline: goes through the real macro entry `expand_attr_macro` (constant expansion →
    /// angle_collect → instruction preprocessing → where rewrite → `A<>` copying →
    /// parse/expand → generate impl), no panic on any input. Uses a fixed dummy trait as the
    /// signature source of truth; directives in random tokens may fail to find an item
    /// (reported via `compile_error!`) or produce invalid types (passed through as garbage),
    /// all accepted — the promise is "no panic". Reusing the real entry ensures fuzz covers
    /// exactly the same path as production (a handwritten pipeline used to miss constant
    /// expansion and `A<>` copying).
    #[test]
    fn full_pipeline_no_panic(toks in tokens(3)) {
        let ts = toks.iter().map(to_token).collect::<TokenStream>();
        let trait_def: syn::ItemTrait = syn::parse_quote! {
            trait Fuzz { fn m(&self) -> u32; }
        };
        let _ = expand_attr_macro(ts, trait_def, false);
    }

    /// Full pipeline through the impl entry (`expand_impl_entry`):
    /// random attr tokens fed against a fixed dummy impl — the no-panic
    /// promise covers the impl branch of the top-level dispatch too (the
    /// `;` spec split, `@trait` replacement, shape matching and assembly
    /// all run on adversarial input).
    #[test]
    fn impl_entry_full_pipeline_no_panic(toks in tokens(3)) {
        let ts = toks.iter().map(to_token).collect::<TokenStream>();
        let impl_item: syn::ItemImpl = syn::parse_quote! {
            impl FuzzImpl for Wrap<T> { fn m(&self) -> u32 { 0 } }
        };
        let _ = crate::entry::expand_impl_entry(ts, impl_item);
    }
}

/// Regression: a single-token `#blanket` wrapper (`{}` alone) used to
/// underflow `current.len() - 2` in the wrapper parser on debug builds
/// (panic). The guard must surface a diagnostic or an expansion — never
/// a panic.
#[test]
fn blanket_single_group_wrapper_no_panic() {
    let attr: TokenStream = "#blanket(@all_methods){{}}".parse().unwrap();
    let trait_def: syn::ItemTrait = syn::parse_quote! {
        trait BlanketBug { fn m(&self); }
    };
    let _ = expand_attr_macro(attr, trait_def, true);
}
