#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))]
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/tutorial.md"))]
// The library uses no unsafe **in its own logic** — enforced as deny rather
// than forbid for exactly one audited exception: the test-build allocation
// guard (`testing::GuardAlloc`), which turns runaway fuzz allocations into
// catchable panics instead of process aborts.
#![deny(unsafe_code)]
#![deny(missing_docs)]
// The `delimiter!` macro is defined at the top of preprocess and imported into the crate
// root via `#[macro_use]`; textual scope requires its declaration to precede all users
// (fuzz / parse / this module).
#[macro_use]
pub(crate) mod preprocess;
#[cfg(test)]
mod testing;
#[cfg(test)]
#[global_allocator]
static FUZZ_GUARD: testing::GuardAlloc = testing::GuardAlloc;
use syn::{ItemTrait, parse_macro_input};

mod analyze;
mod apply;
mod ast;
mod codegen;
mod entry;
mod parse;
mod util;

pub(crate) use analyze::TraitBounds;
pub(crate) use entry::{expand_attr_macro, expand_batch_trait};

/// Attribute macro that generates `impl` blocks for a trait in batch.
///
/// Annotate a trait definition with `#[batch_impl(...)]`; every impl-spec in the macro
/// arguments generates a corresponding `impl` block for that trait.
///
/// ## Syntax
///
/// ```text
/// #[batch_impl( impl-spec [, impl-spec]* [{ body }]? )]
/// ```
///
/// An impl-spec has three parts (the tail of each part may be omitted):
/// - `<impl generics>` — generic params of the `impl` block
/// - `Trait name<trait generics>` — the trait's generic args and associated type bindings
/// - target type — wrapped in `[]` for a parallel list, `.`/space for generic application
///
/// ## The impl entry (0.8.0)
///
/// The same attribute also accepts an `impl` block: the DSL describes a shape
/// template × matrix source, and every matrix leaf instantiates the impl
/// (the slot mapping rewrites the for-Type / where / body; the original impl
/// is withheld):
///
/// ```
/// # use batch_impl::batch_impl;
/// # use std::rc::Rc;
/// # trait Mk { fn make() -> Self; }
/// #[batch_impl(Wrapper<T> : [Box, Rc].u8)]
/// impl Mk for Wrapper<T> { fn make() -> Wrapper<T> { Wrapper::new(T::default()) } }
/// // → impl Mk for Box<u8> { fn make() -> Box<u8> { Box::new(u8::default()) } }
/// // → impl Mk for Rc<u8>  { fn make() -> Rc<u8>  { Rc::new(u8::default()) } }
/// ```
///
/// ## Examples
///
/// ```
/// # use batch_impl::batch_impl;
/// #[batch_impl(usize, isize)]
/// trait Numeric {}
///
/// #[batch_impl(<T> Vec<T>)]
/// trait Collection {}
///
/// #[batch_impl(<T> FromValue<T> [i32 { fn wrap(_: T) -> Self { 0 }}, u32 #wrap{0}] )]
/// trait FromValue<T> { fn wrap(val: T) -> Self; }
///
/// // #name{body} also supports const and type items
/// #[batch_impl(usize #MY_CONST{42})]
/// trait HasConst { const MY_CONST: usize; }
///
/// ```
#[proc_macro_attribute]
pub fn batch_impl(
    attr: proc_macro::TokenStream, item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    // impl entry (0.8.0): the attribute also accepts an `impl` block — batch
    // instantiation from a shape-template × matrix-source description. The
    // trait branch is untouched (top-level dispatch only). Dispatch on the
    // first semantic token (`impl` → impl entry, otherwise → trait entry) so
    // the common trait path parses `syn::ItemTrait` exactly once; a malformed
    // input still gets the shared misuse diagnostic.
    if starts_with_impl(&item) {
        match syn::parse::<syn::ItemImpl>(item) {
            Ok(impl_item) => entry::expand_impl_entry(attr.into(), impl_item)
                .map(proc_macro::TokenStream::from)
                .unwrap_or_else(Into::into),
            Err(_) => entry_misuse_error(),
        }
    } else {
        match syn::parse::<ItemTrait>(item) {
            Ok(trait_item) => expand_attr_macro(attr.into(), trait_item, true)
                .map(proc_macro::TokenStream::from)
                .unwrap_or_else(Into::into),
            Err(_) => entry_misuse_error(),
        }
    }
}

/// Whether the macro input is an impl block: scan top-level tokens past
/// attributes (`#[...]`), visibility (`pub` / `pub(crate)`), and the
/// `unsafe` / `auto` / `default` modifiers — the first semantic ident decides
/// (`impl` vs anything else, e.g. `trait`). Body content (nested `impl Trait`
/// in signatures) lives inside groups and never reaches this scan.
fn starts_with_impl(item: &proc_macro::TokenStream) -> bool {
    let mut iter = item.clone().into_iter().peekable();
    while let Some(t) = iter.next() {
        match t {
            // `#[...]` attribute
            proc_macro::TokenTree::Punct(p) if p.as_char() == '#' => {
                iter.next(); // the `[...]` group
            }
            // `pub` — a `(crate)` / `(super)` / `(in path)` group may follow
            proc_macro::TokenTree::Ident(id) if id.to_string() == "pub" => {
                if matches!(iter.peek(), Some(proc_macro::TokenTree::Group(_))) {
                    iter.next();
                }
            }
            // `unsafe` / `auto` / `default` modifiers
            proc_macro::TokenTree::Ident(id)
                if matches!(id.to_string().as_str(), "unsafe" | "auto" | "default") => {}
            proc_macro::TokenTree::Ident(id) => return id.to_string() == "impl",
            _ => return false,
        }
    }
    false
}

/// Shared misuse diagnostic for an input that is neither a trait nor an impl.
fn entry_misuse_error() -> proc_macro::TokenStream {
    proc_macro::TokenStream::from(util::compile_error_str(
        "batch-impl: expected a trait definition (`trait ...`) or an impl block \
         (`impl Trait for Type { ... }`)",
        proc_macro2::Span::call_site(),
    ))
}

#[doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/doc/batch_impl_only.md"))]
#[proc_macro_attribute]
pub fn batch_impl_only(
    attr: proc_macro::TokenStream, item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let trait_item = parse_macro_input!(item as ItemTrait);
    expand_attr_macro(attr.into(), trait_item, false)
        .map(proc_macro::TokenStream::from)
        .unwrap_or_else(Into::into)
}

#[doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/doc/batch_trait.md"))]
#[proc_macro]
pub fn batch_trait(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    expand_batch_trait(input).unwrap_or_else(Into::into)
}

#[doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/doc/batch_preprocess_test.md"))]
#[proc_macro]
pub fn batch_preprocess_test(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    entry::preprocess_test(input.into())
        .map(proc_macro::TokenStream::from)
        .unwrap_or_else(Into::into)
}

#[doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/doc/batch_preview.md"))]
#[proc_macro]
pub fn batch_preview(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    entry::preview(input.into()).map(proc_macro::TokenStream::from).unwrap_or_else(Into::into)
}

// ============================================================
// Documentation placeholders for the DSL directive / macro-meta layers.
//
// The `#` directives and `@` constants live inside macro arguments, so IDE
// hover and docs.rs cannot reach them. Each placeholder below is a public
// no-op function whose doc block documents one directive — a hoverable,
// searchable rustdoc entry. They are not callable: invoking one reports the
// documentation-only status instead of silently expanding to nothing.
// ============================================================

#[doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/doc/directive_delegate.md"))]
#[proc_macro]
pub fn batch_impl_delegate(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    proc_macro::TokenStream::from(util::compile_error_str(
        "batch-impl: `batch_impl_delegate!` is a documentation-only entry point \
         for the `#delegate` directive — use `#delegate(methods){target}` inside \
         `#[batch_impl(...)]` instead",
        proc_macro2::Span::call_site(),
    ))
}

#[doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/doc/directive_fill.md"))]
#[proc_macro]
pub fn batch_impl_fill(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    proc_macro::TokenStream::from(util::compile_error_str(
        "batch-impl: `batch_impl_fill!` is a documentation-only entry point \
         for the `#fill` directive — use `#fill(methods){body}` inside \
         `#[batch_impl(...)]` instead",
        proc_macro2::Span::call_site(),
    ))
}

#[doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/doc/directive_blanket.md"))]
#[proc_macro]
pub fn batch_impl_blanket(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    proc_macro::TokenStream::from(util::compile_error_str(
        "batch-impl: `batch_impl_blanket!` is a documentation-only entry point \
         for the `#blanket` directive — use `#blanket(methods){wrapper matrix}` \
         inside `#[batch_impl(...)]` instead",
        proc_macro2::Span::call_site(),
    ))
}

#[doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/doc/directive_name.md"))]
#[proc_macro]
pub fn batch_impl_name(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    proc_macro::TokenStream::from(util::compile_error_str(
        "batch-impl: `batch_impl_name!` is a documentation-only entry point \
         for the `#name` directive — use `#name{body}` inside \
         `#[batch_impl(...)]` instead",
        proc_macro2::Span::call_site(),
    ))
}

#[doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/doc/directive_open.md"))]
#[proc_macro]
pub fn batch_impl_open(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    proc_macro::TokenStream::from(util::compile_error_str(
        "batch-impl: `batch_impl_open!` is a documentation-only entry point \
         for the open-extension protocol — write your own `#name(args){body}` \
         macro instead (see `batch_preprocess_test!` for a reference \
         implementation)",
        proc_macro2::Span::call_site(),
    ))
}

#[doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/doc/directive_consts.md"))]
#[proc_macro]
pub fn batch_impl_consts(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    proc_macro::TokenStream::from(util::compile_error_str(
        "batch-impl: `batch_impl_consts!` is a documentation-only entry point \
         for the `@` constant system — write `@name=value;` sections directly \
         (only `batch_trait!` supports custom constants)",
        proc_macro2::Span::call_site(),
    ))
}
