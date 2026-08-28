//! Golden expansion snapshots: representative specs' **final rendered
//! output** locked against `tests/golden/*.golden`. This is the one
//! regression layer the feature/UI tests cannot provide — the token-level
//! consistency tests (`regression_consistency`) cross-check two entries
//! against each other, and the single UI pass fixture only asserts that the
//! output compiles. A golden file pins the exact rendered text: any drift in
//! the render pipeline (spacing, ordering, fresh naming, where placement)
//! shows up here as a one-line diff instead of hiding behind "still
//! compiles".
//!
//! Run with `BLESS=1 cargo test --lib golden` to (re)write the snapshots
//! after an intentional render change; the plain run asserts equality.

use proc_macro2::{Delimiter, Group, TokenStream, TokenTree};
use std::path::PathBuf;
use syn::{ItemImpl, ItemTrait};

use crate::entry::{expand_attr_macro, expand_impl_entry};

/// The canonical specs: each covers one render-shape family. Names become
/// `tests/golden/<name>.golden`.
struct Spec {
    name: &'static str,
    attr: &'static str,
    trait_src: &'static str,
}

const SPECS: &[Spec] = &[
    Spec { name: "matrix", attr: "[Box, Rc] [u8, u16]", trait_src: "trait M {}" },
    Spec { name: "splat", attr: "Pair3<*[Box, Rc]>", trait_src: "trait S {}" },
    Spec { name: "nested_apply", attr: "Box Vec u8", trait_src: "trait N {}" },
    Spec { name: "where_clause", attr: "<T: Clone> Holder<T> Box<T>", trait_src: "trait W {}" },
    Spec {
        name: "directive",
        attr: "[u8, u16] #name{\"n\"}",
        trait_src: "trait D { fn name(&self) -> &'static str; }",
    },
];

/// The impl-entry snapshots: the same output-shape families through the
/// ItemImpl entry (`impl{...}` shape templates, fresh generics).
struct ImplSpec {
    name: &'static str,
    attr: &'static str,
    impl_src: &'static str,
}

const IMPL_SPECS: &[ImplSpec] = &[
    ImplSpec {
        name: "impl_entry_shape",
        attr: "Wrapper<T> : [Box, Rc].u8",
        impl_src: "impl Make for Wrapper<T> { fn make() -> Wrapper<T> { Wrapper::new(T::default()) } }",
    },
    ImplSpec {
        name: "impl_entry_generics",
        attr: "A<B> : [Box, Rc] [u8, u16]",
        impl_src: "impl Conv<B> for A<B> where B: Clone { fn into_b(self) -> B { self.0 } }",
    },
];

fn expand_impl_spec(spec: &ImplSpec) -> String {
    let attr: TokenStream = spec.attr.parse().expect("attr parses");
    let item: ItemImpl = syn::parse_str(spec.impl_src).expect("impl parses");
    let out = expand_impl_entry(attr, item).expect("impl-entry spec expands without error");
    render(&out)
}

#[test]
fn golden_impl_entry_snapshots_match() {
    let bless = std::env::var("BLESS").is_ok();
    for spec in IMPL_SPECS {
        let actual = expand_impl_spec(spec);
        let path = golden_path(spec.name);
        if bless {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, actual).unwrap();
        } else {
            let expected = std::fs::read_to_string(&path).unwrap_or_else(|e| {
                panic!("golden file missing for `{}` ({e}) — run with BLESS=1 to create", spec.name)
            });
            assert_eq!(
                expected, actual,
                "golden snapshot `{}` drifted — inspect `cargo expand`-style diff; \
                 BLESS=1 to accept the new output",
                spec.name
            );
        }
    }
}

/// Renders a token stream as deterministic, newline-delimited text — the
/// same input always renders the same string (token order + group nesting
/// are the only data; spans are not part of `to_string`).
fn render(ts: &TokenStream) -> String {
    render_tokens(&ts.clone().into_iter().collect::<Vec<_>>(), 0)
}

fn render_tokens(tokens: &[TokenTree], depth: usize) -> String {
    let mut out = String::new();
    let pad = "  ".repeat(depth);
    for t in tokens {
        match t {
            TokenTree::Group(g) => {
                out.push_str(&format!("{pad}{}\n", group_open(g)));
                out.push_str(&render_tokens(
                    &g.stream().into_iter().collect::<Vec<_>>(),
                    depth + 1,
                ));
                out.push_str(&format!("{pad}{}\n", group_close(g)));
            }
            other => out.push_str(&format!("{pad}{other}\n")),
        }
    }
    out
}

fn group_open(g: &Group) -> &'static str {
    match g.delimiter() {
        Delimiter::Parenthesis => "(",
        Delimiter::Brace => "{",
        Delimiter::Bracket => "[",
        Delimiter::None => "<>",
    }
}

fn group_close(g: &Group) -> &'static str {
    match g.delimiter() {
        Delimiter::Parenthesis => ")",
        Delimiter::Brace => "}",
        Delimiter::Bracket => "]",
        Delimiter::None => "</>",
    }
}

/// Expands a spec through the real attribute pipeline and returns the
/// rendered text (trait + impls, as the macro emits them).
fn expand_spec(spec: &Spec) -> String {
    let attr: TokenStream = spec.attr.parse().expect("attr parses");
    let item: ItemTrait = syn::parse_str(spec.trait_src).expect("trait parses");
    let out = expand_attr_macro(attr, item, true).expect("spec expands without error");
    render(&out)
}

fn golden_path(name: &str) -> PathBuf {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden");
    PathBuf::from(dir).join(format!("{name}.golden"))
}

#[test]
fn golden_snapshots_match() {
    let bless = std::env::var("BLESS").is_ok();
    for spec in SPECS {
        let actual = expand_spec(spec);
        let path = golden_path(spec.name);
        if bless {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, actual).unwrap();
        } else {
            let expected = std::fs::read_to_string(&path).unwrap_or_else(|e| {
                panic!("golden file missing for `{}` ({e}) — run with BLESS=1 to create", spec.name)
            });
            assert_eq!(
                expected, actual,
                "golden snapshot `{}` drifted — inspect `cargo expand`-style diff; \
                 BLESS=1 to accept the new output",
                spec.name
            );
        }
    }
}
