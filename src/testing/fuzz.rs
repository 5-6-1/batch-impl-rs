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

/// The alphabet must be able to spell the shapes the recorded failures needed, and this drives
/// the real generator rather than a copy of its list. The drawing is seeded and runs four
/// thousand cases rather than the default 256 on purpose: a sampled assertion at that size
/// flaked eight times in three thousand runs while these tokens were missing.
///
/// The drawn-token checks below can only spot-check members, so they are paired with checks on
/// [`ALPHABET`] itself: the table the generator draws from is asserted to be free of duplicate
/// entries and to contain each shape. A token deleted from the table fails here even if the
/// sample never happened to need it.
#[test]
fn the_alphabet_can_spell_the_shapes_that_once_escaped_it() {
    use proptest::test_runner::{Config, TestRunner};

    let mut seen = std::collections::BTreeSet::new();
    for t in ALPHABET {
        assert!(seen.insert(format!("{t:?}")), "duplicate alphabet entry: {t:?}");
    }
    assert_eq!(
        seen.len(),
        ALPHABET.len(),
        "duplicate alphabet entry (the distinct set is smaller than the table)"
    );
    // The count is deliberate, like the corpus floors in `tests/no_panic`: adding or removing a
    // token is a decision, and this makes it one. Without it, deleting a token that no fixed
    // shape check names (the six directive words, `impl`, `@`) stayed invisible - which is what
    // probe C measured by deletion. `seen` above already proves the entries are distinct.
    assert_eq!(
        ALPHABET.len(),
        52,
        "the alphabet changed size: was a token added or removed deliberately?"
    );
    assert!(
        ALPHABET.iter().any(|t| matches!(t, Tok::Punct('-', Spacing::Joint))),
        "the alphabet has no Joint `-`: `->` cannot be generated at all"
    );
    assert!(
        ALPHABET.iter().any(|t| matches!(t, Tok::Literal(l) if l.starts_with('"'))),
        "the alphabet has no string literal"
    );
    assert!(
        ALPHABET.iter().any(|t| matches!(t, Tok::Ident("extern") | Tok::Ident("dyn"))),
        "the alphabet has no extern/dyn keyword"
    );
    // The idents, pinned as a set. `52` plus the three shape classes leaves a hole probe C's G7
    // walked through: substituting one keyword for another (`for` -> `from`) kept the size, kept the
    // distinctness and kept every shape assertion, so a typo in the vocabulary stayed invisible. A
    // set comparison is the only check that notices a swap rather than a removal.
    let mut idents: Vec<String> = ALPHABET
        .iter()
        .filter_map(|t| match t {
            Tok::Ident(name) => Some(name.to_string()),
            _ => None,
        })
        .collect();
    idents.sort();
    let expected = [
        "Box", "Cow", "Self", "T", "Vec", "all", "blanket", "call", "const", "delegate", "dyn",
        "extern", "f64", "fill", "fn", "for", "i32", "impl", "isize", "mut", "name", "r#type",
        "r#value", "self", "trait", "u8", "unsafe", "usize", "where",
    ];
    assert_eq!(
        idents, expected,
        "the alphabet's idents changed: a keyword was swapped, added or removed — that is a \
         deliberate decision, so update this list in the same commit"
    );

    let mut runner = TestRunner::new(Config { cases: 4000, ..Config::default() });
    // `run` takes an `Fn`, so what a drawing saw is recorded through a `Cell` rather than a
    // captured `mut` binding.
    let arrow = std::cell::Cell::new(false);
    let string = std::cell::Cell::new(false);
    let keyword = std::cell::Cell::new(false);
    let outcome = runner.run(&tokens(0), |toks| {
        for t in &toks {
            match t {
                Tok::Punct('-', Spacing::Joint) => arrow.set(true),
                Tok::Literal(l) if l.starts_with('"') => string.set(true),
                Tok::Ident("extern") | Tok::Ident("dyn") => keyword.set(true),
                _ => {}
            }
        }
        Ok(())
    });
    assert!(outcome.is_ok(), "the generator itself failed: {outcome:?}");
    assert!(arrow.get(), "no Joint `-` in 4000 drawings: `->` cannot be generated at all");
    assert!(string.get(), "no string literal in 4000 drawings");
    assert!(keyword.get(), "no extern/dyn keyword in 4000 drawings");
}

/// The generator's leaf vocabulary. It is a named table rather than an inline strategy because
/// the assertion below has to be able to talk about *exactly this set*: a `prop_oneof!` list
/// cannot be enumerated, so an assertion written against one could only ever spot-check a few
/// members, and deleting a token went unnoticed. Generator and assertion now share this table.
const ALPHABET: &[Tok] = &[
    // DSL / Rust keywords and common type names
    Tok::Ident("usize"),
    Tok::Ident("isize"),
    Tok::Ident("r#type"),
    Tok::Ident("r#value"),
    Tok::Ident("Vec"),
    Tok::Ident("Box"),
    Tok::Ident("T"),
    Tok::Ident("where"),
    Tok::Ident("fn"),
    Tok::Ident("self"),
    Tok::Ident("unsafe"),
    // Directive words: drive the `#` directive and open-extension paths
    // in the full-pipeline fuzz (the no-panic promise covers them too).
    Tok::Ident("blanket"),
    Tok::Ident("fill"),
    Tok::Ident("delegate"),
    Tok::Ident("call"),
    Tok::Ident("name"),
    Tok::Ident("all"),
    // The `impl` keyword: makes `impl{...}` templates reachable, so the
    // variadic-segment marking pass (`mark_varseg` → `mark_template`,
    // whose postcondition reports a residue instead of panicking) is
    // actually exercised by the random corpus — without this ident the
    // pass was never entered and the residue guard had no fuzz coverage.
    Tok::Ident("impl"),
    // Constant-system words: built-in families / range endpoints / the
    // `@trait` marker / blanket's `@Cow` — the `@` punct below can now
    // reach the constant expansion, range, and lifetime paths.
    Tok::Ident("u8"),
    Tok::Ident("i32"),
    Tok::Ident("f64"),
    Tok::Ident("Cow"),
    Tok::Ident("trait"),
    Tok::Ident("Self"),
    // Numeric literals (small-integer DSL exponents)
    Tok::Literal("0"),
    Tok::Literal("1"),
    Tok::Literal("3"),
    // DSL operators and punctuation
    Tok::Punct('<', Spacing::Alone),
    Tok::Punct('>', Spacing::Alone),
    Tok::Punct('.', Spacing::Alone),
    Tok::Punct('-', Spacing::Alone),
    Tok::Punct(',', Spacing::Alone),
    Tok::Punct(';', Spacing::Alone),
    Tok::Punct(':', Spacing::Alone),
    // A Joint `:` can combine with the next `:` into `::`
    Tok::Punct(':', Spacing::Joint),
    Tok::Punct('&', Spacing::Alone),
    Tok::Punct('*', Spacing::Alone),
    Tok::Punct('#', Spacing::Alone),
    Tok::Punct('!', Spacing::Alone),
    Tok::Punct('=', Spacing::Alone),
    // `@` constants, `..`/`..=` ranges (Joint `.` heads a range), `'`
    // lifetimes, and bound/bound-start punctuation — the paths the old
    // vocabulary could never reach.
    Tok::Punct('@', Spacing::Alone),
    Tok::Punct('.', Spacing::Joint),
    Tok::Punct('+', Spacing::Alone),
    Tok::Punct('?', Spacing::Alone),
    Tok::Punct('\'', Spacing::Alone),
    // Shapes the vocabulary could not spell at all, so whole paths went unexercised:
    // `->` needs a **Joint** `-` (there was only an Alone one), and the recorded fuzz
    // failure for `Op::Arrow` was therefore misread as a parser gap rather than the
    // alphabet's. A string literal and the `extern` / `dyn` / `for` / `const` / `mut`
    // keywords close the same class (probe C, measured by absence).
    Tok::Punct('-', Spacing::Joint),
    Tok::Literal("\"C\""),
    Tok::Ident("extern"),
    Tok::Ident("dyn"),
    Tok::Ident("for"),
    Tok::Ident("const"),
    Tok::Ident("mut"),
];

/// Depth-limited token list generator (covers DSL keywords, operators, bracket nesting)
fn tokens(depth: usize) -> impl Strategy<Value = Vec<Tok>> {
    let leaf = prop::sample::select(ALPHABET.to_vec());
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
        Tok::Ident(s) => match s.strip_prefix("r#") {
            Some(rest) => Ident::new_raw(rest, proc_macro2::Span::call_site()).into(),
            None => Ident::new(s, proc_macro2::Span::call_site()).into(),
        },
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
        while parse_item(&mut cursor, Op::Comma, crate::parse::Ctx::default()).is_some() {}
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

/// The shape kernel's bindings, which the impl entry's leaf check reads: a template whose ident differs
/// from the leaf's binds its own name (`A => u8`), a template identical to the leaf binds nothing, and a
/// placeholder the impl writes binds too (`W => u8`). Pinned because the leaf-ignored diagnostic turns on
/// that distinction — with an empty mapping the only way a leaf can be used is by *being* the produced
/// for-type, which is exactly how the legal idempotent spellings (`Wrap<N> : Wrap<N>`, `A : A`) pass.
#[test]
fn shape_kernel_bindings_behind_the_leaf_check() {
    let cases: [(&str, &str, &[&str]); 4] = [
        ("A", "u8", &["A => u8"]),
        ("A", "u16", &["A => u16"]),
        ("Vec<u8>", "Vec<u8>", &[]),
        ("W", "u8", &["W => u8"]),
    ];
    for (tpl, leaf, want) in cases {
        let template: syn::Type = syn::parse_str(tpl).unwrap();
        let leaf_ty: syn::Type = syn::parse_str(leaf).unwrap();
        let (m, _) =
            crate::codegen::match_shape(&template, &leaf_ty, &std::collections::HashSet::new())
                .expect("the shape matches");
        let got = m.slots().iter().map(|(n, v)| format!("{n} => {v}")).collect::<Vec<_>>();
        assert_eq!(got, want, "{tpl} vs {leaf}");
    }
}

/// Regression (alga2, round 10): a **varseg template written with a trailing comma**
/// (`impl{(A@..,)}`) used together with a `where` clause produced a malformed type — the body landed
/// where a type belongs and rustc reported ``expected type, found `{` `` with nothing pointing at the
/// spec. The template alone works, and so does the comma-less spelling with a where clause.
#[test]
fn varseg_comma_template_with_a_where_clause_keeps_the_body_out_of_the_type() {
    let attr: TokenStream =
        "(u8, u16) ().1..=2 where @0..: Clone impl{(A@..,)} #n{0}".parse().unwrap();
    let trait_def: syn::ItemTrait = syn::parse_quote! {
        trait TrV { fn n(&self) -> usize; }
    };
    let out = match expand_attr_macro(attr, trait_def, true) {
        Ok(ts) => ts.to_string(),
        Err(e) => e.to_string(),
    };
    println!("EXPANSION(A): {out}");
    assert!(
        !out.contains("for {") && !out.contains("::core::compile_error!"),
        "the spec did not expand into a type: {out}"
    );
}

/// Regression (alga2, round 10): a bare `where` region ending in a **trailing comma** before the
/// spec's body (`where T: Clone, { … }`) left that comma in the stream, so the body became a spec of
/// its own — "a bare `{...}` block without an attached type generates no impl". Working in 0.9.6.
#[test]
fn bare_where_trailing_comma_keeps_the_body_attached() {
    let attr: TokenStream =
        "<T: Clone> TrW<T> T where T: Clone, { fn f(&self) -> u8 { 0 } }".parse().unwrap();
    let trait_def: syn::ItemTrait = syn::parse_quote! {
        trait TrW<T> { fn f(&self) -> u8; }
    };
    let out = match expand_attr_macro(attr, trait_def, true) {
        Ok(ts) => ts.to_string(),
        Err(e) => e.to_string(),
    };
    println!("EXPANSION(B): {out}");
    assert!(
        !out.contains("without an attached type"),
        "the body was taken as a spec of its own: {out}"
    );
}
