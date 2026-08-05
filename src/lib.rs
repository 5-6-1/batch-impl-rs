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
use proc_macro2::{TokenStream, TokenTree};
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

use preprocess::{build_from_item, get_trait_item, parse_names_from_tokens};
use util::compile_error_str;

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

/// Same as `#[batch_impl]`, but discards the annotated trait definition and only emits
/// `impl` blocks.
///
/// For traits already defined elsewhere where only batched impl generation is needed. The
/// annotated trait merely serves as the "signature source of truth" for the directive system:
/// `#name`/`#fill`/`#delegate` read item signatures from it, and the open extension
/// `#name(args){body}` hands (method name list, body, the whole trait) to the user's
/// same-named function-like macro (see README "Directive System"). The syntax is identical
/// to `#[batch_impl]`.
///
/// ## Examples
///
/// ```
/// # use batch_impl::batch_impl_only;
/// trait Greet { fn hello(&self) -> &str; }
///
/// #[batch_impl_only(usize #hello{"hi"})]
/// trait Greet { fn hello(&self) -> &str; } // this trait definition is dropped, existing definitions are unaffected
/// // Written with batch_impl_only instead of batch_trait to use the directive system; write it verbatim at the trait definition site
/// ```
#[proc_macro_attribute]
pub fn batch_impl_only(
    attr: proc_macro::TokenStream, item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let trait_item = parse_macro_input!(item as ItemTrait);
    expand_attr_macro(attr.into(), trait_item, false)
        .map(proc_macro::TokenStream::from)
        .unwrap_or_else(Into::into)
}

/// Function-like macro that generates `impl` blocks for a declared trait in batch.
///
/// Syntax: `unsafe? Trait path: impl-specs;`, with `;` separating multiple trait segments.
/// After each segment's `:` comes a DSL expression (type DSL + `@` constants, same as
/// `#[batch_impl]`).
///
/// **`#` directives are not supported** (`#fill`/`#delegate`/`#blanket`/open extension):
/// directives need the trait definition as the signature source of truth, which `batch_trait!`
/// as a function-like macro cannot access; use `#[batch_impl]` / `#[batch_impl_only]` when
/// you need directives.
///
/// ## Examples
///
/// ```
/// # use batch_impl::batch_trait;
/// trait A {}
/// trait B<T> {}
/// unsafe trait UnsafeTrait{}
///
/// batch_trait!(
///     A: usize, isize;
///     B: <T> B<T> Vec<T>;
///     unsafe UnsafeTrait: usize
/// );
/// ```
///
/// Path traits (such as `foo::C`) are supported too; see tests/regression.rs.
#[proc_macro]
pub fn batch_trait(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    expand_batch_trait(input).unwrap_or_else(Into::into)
}

/// Test-only open-extension macro (function-like): `name!{(method name list){body} trait T {...}}`.
///
/// Parses the method name list, body, and trait definition from the macro input, generating
/// `fn signature { body }` per method (reusing the trait signature) — equivalent to handing
/// the `#fill` implementation to the user.
///
/// Used to verify open instruction extension: `#name(args){body}` expands to
/// `{name!{(args){body} trait ...}}`, with the macro call landing in the impl body and being
/// expanded by the user macro into the needed fn definitions based on the trait
/// (see section 28 of `tests/dsl.rs`).
///
/// Design point: this must be a **function-like macro call** `name!{...}`, not an
/// `#[name[...]] trait ...` attribute — a trait is not a valid item inside an impl block
/// (`#[attr] trait` cannot appear in an impl), whereas a function-like macro in an impl
/// body position is expanded by rustc into associated items.
#[doc(hidden)]
#[proc_macro]
pub fn batch_preprocess_test(
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let tokens = TokenStream::from(input).into_iter().collect::<Vec<_>>();
    let tokens = match preprocess::angle_collect(&tokens) {
        Ok(v) => v,
        Err(e) => return e.into(),
    };
    // Shape: `(add, inc) {*self+1} trait AddInc {...}`
    let Some(TokenTree::Group(names_group)) = tokens.first() else {
        return compile_error_str(
            "batch-impl: batch_preprocess_test expects `(method name list){body} trait ...`",
            tokens
                .first()
                .map(|t| t.span())
                .unwrap_or_else(proc_macro2::Span::call_site),
        )
        .into();
    };
    if names_group.delimiter() != delimiter![()] {
        return compile_error_str(
            "batch-impl: batch_preprocess_test expects `(method name list){body} trait ...`",
            tokens
                .first()
                .map(|t| t.span())
                .unwrap_or_else(proc_macro2::Span::call_site),
        )
        .into();
    }
    let Some(TokenTree::Group(body_group)) = tokens.get(1) else {
        return compile_error_str(
            "batch-impl: batch_preprocess_test expects `(method name list){body} trait ...`",
            tokens
                .get(1)
                .map(|t| t.span())
                .unwrap_or_else(proc_macro2::Span::call_site),
        )
        .into();
    };
    if body_group.delimiter() != delimiter![{}] {
        return compile_error_str(
            "batch-impl: batch_preprocess_test expects `(method name list){body} trait ...`",
            tokens
                .get(1)
                .map(|t| t.span())
                .unwrap_or_else(proc_macro2::Span::call_site),
        )
        .into();
    }
    let trait_ts = tokens[2..].iter().cloned().collect();
    let trait_item = match syn::parse2(trait_ts) {
        Ok(t) => t,
        Err(_) => {
            return compile_error_str(
                "batch-impl: batch_preprocess_test cannot parse the trait definition",
                proc_macro2::Span::call_site(),
            )
            .into();
        }
    };
    let names = match parse_names_from_tokens(
        &names_group.stream().into_iter().collect::<Vec<_>>(),
        &trait_item,
    ) {
        Ok(names) => names,
        Err(e) => return e.into(),
    };
    let body = body_group.stream();
    let mut methods = TokenStream::new();
    for name in &names {
        let item = match get_trait_item(&trait_item, name) {
            Ok(item) => item,
            Err(e) => return e.into(),
        };
        methods.extend(build_from_item(item, &body));
    }
    preprocess::render_angles(methods).into()
}
