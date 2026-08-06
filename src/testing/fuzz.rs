//! Property-based testing (proptest) with a no-panic property.
//!
//! The library's promise is "no panic on user input". Feed random token sequences to the
//! dangerous entry points and assert that no input panics — `Err` / `None` / `compile_error!`
//! results are all accepted. Coverage: bare where rewrite, DSL parsing, and the **full
//! pipeline** (instruction preprocessing → where rewrite → parse/expand → generate impl,
//! incl. apply/expand/codegen).

use proc_macro2::{
    Delimiter, Group, Ident, Literal, Punct, Spacing, TokenStream, TokenTree,
};
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
        prop::strategy::Just(Tok::Ident("usize")),
        prop::strategy::Just(Tok::Ident("isize")),
        prop::strategy::Just(Tok::Ident("Vec")),
        prop::strategy::Just(Tok::Ident("Box")),
        prop::strategy::Just(Tok::Ident("T")),
        prop::strategy::Just(Tok::Ident("where")),
        prop::strategy::Just(Tok::Ident("fn")),
        prop::strategy::Just(Tok::Ident("self")),
        prop::strategy::Just(Tok::Ident("unsafe")),
        // Numeric literals (small-integer DSL exponents)
        prop::strategy::Just(Tok::Literal("0")),
        prop::strategy::Just(Tok::Literal("1")),
        prop::strategy::Just(Tok::Literal("3")),
        // DSL operators and punctuation
        prop::strategy::Just(Tok::Punct('<', Spacing::Alone)),
        prop::strategy::Just(Tok::Punct('>', Spacing::Alone)),
        prop::strategy::Just(Tok::Punct('^', Spacing::Alone)),
        prop::strategy::Just(Tok::Punct('-', Spacing::Alone)),
        prop::strategy::Just(Tok::Punct(',', Spacing::Alone)),
        prop::strategy::Just(Tok::Punct(';', Spacing::Alone)),
        prop::strategy::Just(Tok::Punct(':', Spacing::Alone)),
        // A Joint `:` can combine with the next `:` into `::`
        prop::strategy::Just(Tok::Punct(':', Spacing::Joint)),
        prop::strategy::Just(Tok::Punct('&', Spacing::Alone)),
        prop::strategy::Just(Tok::Punct('*', Spacing::Alone)),
        prop::strategy::Just(Tok::Punct('#', Spacing::Alone)),
        prop::strategy::Just(Tok::Punct('!', Spacing::Alone)),
        prop::strategy::Just(Tok::Punct('=', Spacing::Alone)),
    ];
    if depth == 0 {
        prop::collection::vec(leaf, 0..6).boxed()
    } else {
        let grouped = prop_oneof![
            prop::strategy::Just(delimiter![()]),
            prop::strategy::Just(delimiter![[]]),
            prop::strategy::Just(delimiter![{}]),
            // Real None groups simulate macro-variable expansion output — angle_collect
            // should flatten them (contents are DSL tokens)
            prop::strategy::Just(delimiter![none]),
        ]
        .prop_flat_map(move |d| {
            tokens(depth - 1).prop_map(move |inner| Tok::Group(d, inner))
        });
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

proptest! {
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
}
