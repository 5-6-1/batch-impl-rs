#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))]
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/tutorial.md"))]
// The library uses no unsafe; missing docs are rejected as errors (only for pub items;
// internal pub(crate) is exempt).
#![forbid(unsafe_code)]
#![deny(missing_docs)]
// The MSVC linker prints "creating library ... and object ..." to stdout, which rustc
// treats as linker_messages warnings; these are harmless Windows link-product notices,
// suppressed globally.
#![allow(linker_messages)]
// The `delimiter!` macro is defined at the top of preprocess and imported into the crate
// root via `#[macro_use]`; textual scope requires its declaration to precede all users
// (fuzz / parse / this module).
#[macro_use]
pub(crate) mod preprocess;
#[cfg(test)]
mod testing;
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
/// - target type — wrapped in `[]` for a parallel list, `^`/`-` for generic application
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
    let trait_item = parse_macro_input!(item as ItemTrait);
    expand_attr_macro(attr.into(), trait_item, true)
        .map(proc_macro::TokenStream::from)
        .unwrap_or_else(Into::into)
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
#[doc(hidden)]
#[proc_macro]
pub fn batch_preprocess_test(
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    entry::preprocess_test(input.into())
        .map(proc_macro::TokenStream::from)
        .unwrap_or_else(Into::into)
}

// ============================================================
// Documentation placeholders for the DSL directive / macro-meta layers.
//
// The `#` directives and `@` constants live inside macro arguments, so IDE
// hover and docs.rs cannot reach them. Each placeholder below is a public
// no-op function whose doc block documents one directive — a hoverable,
// searchable rustdoc entry. Never call these functions.
// ============================================================

#[doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/doc/directive_delegate.md"))]
#[proc_macro]
pub fn batch_impl_delegate(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    proc_macro::TokenStream::new()
}

#[doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/doc/directive_fill.md"))]
#[proc_macro]
pub fn batch_impl_fill(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    proc_macro::TokenStream::new()
}

#[doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/doc/directive_blanket.md"))]
#[proc_macro]
pub fn batch_impl_blanket(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    proc_macro::TokenStream::new()
}

#[doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/doc/directive_name.md"))]
#[proc_macro]
pub fn batch_impl_name(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    proc_macro::TokenStream::new()
}

#[doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/doc/directive_open.md"))]
#[proc_macro]
pub fn batch_impl_open(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    proc_macro::TokenStream::new()
}

#[doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/doc/directive_consts.md"))]
#[proc_macro]
pub fn batch_impl_consts(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    proc_macro::TokenStream::new()
}
