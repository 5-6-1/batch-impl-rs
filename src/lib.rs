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
use quote::quote;
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

/// Test-only open-extension macro (function-like): `name!{ {spec}(method name list){body} trait T {...} }`.
///
/// Parses the spec body (first Brace group — the target type), the method name list,
/// the body, and the trait definition from the macro input. In the **top-level form**
/// (4 segments) it emits a full `impl Trait for {spec}`; in the legacy in-impl form
/// (3 segments, no spec group) it emits `fn signature { body }` per method (reusing
/// the trait signature) — equivalent to handing the `#fill` implementation to the user.
///
/// Used to verify open instruction extension: `#name(args){body}` expands to
/// `{ ! name!{(args){body} trait ...} }`, the `!` marking top-level emission —
/// codegen prepends the spec body and emits the call at top level, where the user
/// macro generates arbitrary items (typically its own impl)
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
    // Shape: `{spec}(method name list){body} trait ...` (top-level form —
    // the first Brace group is the spec body; the macro emits a full impl
    // for it) or the legacy `(method name list){body} trait ...` (in-impl
    // form — emits associated fn definitions for the enclosing impl).
    let spec = match tokens.first() {
        Some(TokenTree::Group(g))
            if g.delimiter() == delimiter![{}]
                && matches!(
                    tokens.get(1),
                    Some(TokenTree::Group(p)) if p.delimiter() == delimiter![()]
                ) =>
        {
            Some(g.stream())
        }
        _ => None,
    };
    let idx = if spec.is_some() { 1 } else { 0 };
    let Some(TokenTree::Group(names_group)) = tokens.get(idx) else {
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
    let Some(TokenTree::Group(body_group)) = tokens.get(idx + 1) else {
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
    let trait_ts = tokens[idx + 2..].iter().cloned().collect();
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
    match spec {
        // Top-level form: emit a full impl for the spec body (`{spec}` first
        // segment) — the batch_impl crate emits no impl in this mode.
        Some(spec_ts) => {
            let ident = &trait_item.ident;
            preprocess::render_angles(quote!(impl #ident for #spec_ts { #methods }))
                .into()
        }
        None => preprocess::render_angles(methods).into(),
    }
}

// ============================================================
// Documentation placeholders for the DSL directive / macro-meta layers.
//
// The `#` directives and `@` constants live inside macro arguments, so IDE
// hover and docs.rs cannot reach them. Each placeholder below is a public
// no-op function whose doc block documents one directive — a hoverable,
// searchable rustdoc entry. Never call these functions.
// ============================================================

/// Documentation placeholder for the `#delegate` directive.
///
/// `#delegate(args){target}` generates one delegation call per selected
/// method: each becomes `fn m(&self, ...) -> R { (target).m(...) }`. The
/// `self` argument is skipped; the remaining arguments are forwarded (named
/// params as-is, non-identifier patterns renamed to `arg{i}` when they
/// cannot be used as an expression).
///
/// ```
/// # use batch_impl::batch_impl;
/// #[batch_impl(
///     Vec<u32> #d_len{self.len()},
///     Box<Vec<u32>> #delegate(d_len){**self}
/// )]
/// trait MyLen { fn d_len(&self) -> usize; }
/// # fn main() {}
/// ```
///
/// **Documentation marker only — never call this function.**
#[proc_macro]
pub fn batch_impl_delegate(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    proc_macro::TokenStream::new()
}

/// Documentation placeholder for the `#fill` directive.
///
/// `#fill(args){body}` copies each selected trait item's signature and
/// substitutes `body` as its implementation. Selection supports the `@all`
/// families (`@all_methods`, `@all_ref_methods`, `@all_default_methods`,
/// ...), individual names, and `-` subtraction (`#fill(@all_methods, -foo)`).
///
/// ```
/// # use batch_impl::batch_impl;
/// #[batch_impl(Vec<u32> #fill(@all_methods){0})]
/// trait F { fn zero(&self) -> u32; }
/// # fn main() {}
/// ```
///
/// **Documentation marker only — never call this function.**
#[proc_macro]
pub fn batch_impl_fill(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    proc_macro::TokenStream::new()
}

/// Documentation placeholder for the `#blanket` directive.
///
/// `#blanket(args){wrapper list}` implements the trait for every wrapper
/// around a fresh generic `T`, delegating each method by deref. Wrappers may
/// carry a `:N` deref-depth annotation and a `where{...}` predicate; a
/// wrapper whose main part contains `@0` treats `@0` as T's position
/// (`(u32, @0)` → `(u32, T)`), otherwise it is applied as `wrapper^T`.
///
/// ```
/// # use batch_impl::batch_impl;
/// #[batch_impl(#blanket(@all_methods){Box})]
/// trait B { fn tag(&self) -> u32; }
/// # fn main() {}
/// ```
///
/// **Documentation marker only — never call this function.**
#[proc_macro]
pub fn batch_impl_blanket(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    proc_macro::TokenStream::new()
}

/// Documentation placeholder for the `#name{body}` fill-by-name directive.
///
/// `#name{body}` looks up the single trait item named `name` — a method, an
/// associated const, or an associated type — and fills it with `body` (the
/// body must match that item's shape).
///
/// ```
/// # use batch_impl::batch_impl;
/// #[batch_impl(Box<Vec<u32>> #count{self.len()})]
/// trait L { fn count(&self) -> usize; }
/// # fn main() {}
/// ```
///
/// **Documentation marker only — never call this function.**
#[proc_macro]
pub fn batch_impl_name(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    proc_macro::TokenStream::new()
}

/// Documentation placeholder for the open-extension protocol.
///
/// A `#name(args){body}` whose `name` is not a built-in directive expands to
/// a call of a user-defined function-like macro of the same name, handed the
/// args, body and trait definition:
/// `#my_ext(x){y}` → `{ my_ext!{ (x) {y} trait_def } }`.
///
/// ```
/// # use batch_impl::batch_impl;
/// macro_rules! my_ext { ($($rest:tt)*) => {}; }
/// #[batch_impl(Box<u32> #my_ext(x){y})]
/// trait O {}
/// # fn main() {}
/// ```
///
/// **Documentation marker only — never call this function.**
#[proc_macro]
pub fn batch_impl_open(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    proc_macro::TokenStream::new()
}

/// Documentation placeholder for the `@` macro-meta constant system.
///
/// `@` names expand before all other DSL processing (`@ <> # where` order):
/// - built-in name families: `@uint` / `@int` / `@float` / `@num` /
///   `@scalar` and wildcards `@u*` / `@i*` / `@f*`;
/// - range families: `@u8..u128` / `@i8..i128` / `@f32..f64` (inclusive);
/// - `batch_trait!` user constants: a leading `@name = value;` segment
///   (lazy expansion, reference checks);
/// - `@N` position references (resolved by codegen) and `@trait`
///   (segment-level trait path).
///
/// ```
/// # use batch_impl::batch_impl;
/// #[batch_impl(Box^@u*)]
/// trait C {}
/// # fn main() {}
/// ```
///
/// **Documentation marker only — never call this function.**
#[proc_macro]
pub fn batch_impl_consts(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    proc_macro::TokenStream::new()
}
