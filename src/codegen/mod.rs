//! Codegen layer: impl block generation.
//!
//! Recursively dismantles each flattened leaf [`Ty`] (see `lib::parse_batch_trait_entry`)
//! into an [`ImplParts`] (impl generics, trait generics, associated type bindings,
//! target type, body, attrs, unsafe flag), then renders the final
//! `impl<...> Trait<...> for Target { ... }` block.
//!
//! ## Concern layers (order of application, as orchestrated by [`generate_impl`])
//!
//! The codegen pipeline is split by **concern** — each file owns one focused
//! concern that stays within a single processing stage; the *order* is
//! described here (and lives only here), mirroring how `preprocess` documents
//! its pass order:
//!
//! 1. `extract` — `Ty` → [`ImplParts`]: dismantle metadata (`extract_impl_parts`),
//!    substitute trait params in directive bodies (`substitute_trait_generics`),
//!    hoist nested fresh generics (`hoist_type_params`);
//! 2. `splat` — splat expansion on the Ty structure (`expand_splat_elems`), the
//!    deferred flattening of `*()` / `*[]` (they survive parse/apply/expand as
//!    whole units and expand here, one code path for every position);
//! 3. `generics` — impl-generic concerns: same-name declaration merging
//!    (`merge_dup_params`), trait-bound inheritance (`inherit_trait_bounds`),
//!    impl-name normalization (`bare_param_name`);
//! 4. `sync` — `X<>` (empty trait brackets) → the spec's trait application,
//!    with the switch-template body opt-in (`impl{Tr<>}`);
//! 5. `where_at` — where-predicate macro-meta replacement (`@N` → impl generic
//!    N) and dangling-`@` validation;
//! 6. `shape` + `match_ty` — the `impl{...}` shape-template kernel (slot
//!    mapping + variadic segments), and `repeat` (`@(...)..` blocks) +
//!    `repeat_drivers`;
//! 7. `render` — the final `impl<...>` block assembly (`render_impl` +
//!    `collect_shape_mapping`).
//!
//! `top_level` handles the top-level macro form (`{! ...}`); `fresh` the
//! fresh-generic naming context and validation. Tests live beside their
//! concern (`repeat_tests`, `where_at_tests`).

mod bound_gen;
mod extract;
mod fresh;
mod generics;
mod match_ty;
mod pipeline;
mod range_refs;
mod range_worker;
mod render;
mod repeat;
mod repeat_drivers;
#[cfg(test)]
mod repeat_tests;
mod shape;
mod splat;
mod sync;
mod top_level;
mod validate;
mod where_at;
#[cfg(test)]
mod where_at_tests;

pub(crate) use extract::*;
pub(crate) use fresh::*;
pub(crate) use generics::*;
pub(crate) use pipeline::generate_parts;
pub(crate) use repeat::*;
pub(crate) use shape::*;
pub(crate) use splat::*;
pub(crate) use sync::*;
pub(crate) use top_level::*;
pub(crate) use validate::*;
pub(crate) use where_at::*;

use crate::TraitBounds;
use crate::ast::*;
use crate::util::compile_error_str;
use proc_macro2::{Ident, TokenStream, TokenTree};
use quote::ToTokens;

/// Generates one impl block (for a single flattened leaf `Ty`).
///
/// `trait_bounds`: the trait's generic param list (positionally matching the spec's
/// trait arguments). Impl generics **without a bound** inherit by position + same name
/// (`trait Foo<T: Clone>` + `<T> Foo<T>` → `impl<T: Clone>`); mismatched names or
/// bounds referencing undeclared params error out; user-bounded params are untouched
/// (the sub-trait macro cannot infer; writing a bound = user's responsibility).
///
/// Three exits:
/// - `Ty::Error` → output the `compile_error!` stream directly;
/// - bare code block `WithCode(None, ...)` (an open-instruction expansion product) →
///   injected verbatim as a top-level item, not wrapped in an impl;
/// - otherwise → dismantle metadata (`extract_impl_parts`) → hoist nested generics
///   (`hoist_type_params`) → build generics / trait generics / impl body → render `quote!`
pub(crate) fn generate_impl(
    ty: Ty, trait_name: &TokenStream, is_unsafe_trait: bool, trait_bounds: &TraitBounds,
    trait_param_names: &[Ident],
) -> TokenStream {
    // Bare `{...}` as the whole spec. A `!`-marked block (top-level macro
    // form) without an attached type has no spec body to prepend — error
    // instead of emitting invalid Rust (`!` is not an item). Any other bare
    // block (e.g. a `#name{...}` directive expansion) generates no impl —
    // this crate only produces impl blocks, so error instead of emitting the
    // block as a top-level item (top-level injection is the explicit
    // `{! ...}` macro form only).
    if let Ty { kind: TyKind::WithCode(TyWithCode(None, code)), .. } = &ty {
        let is_top_marked = matches!(
            code.0.clone().into_iter().next(),
            Some(TokenTree::Punct(p)) if p.as_char() == '!'
        );
        return compile_error_str(
            if is_top_marked {
                "batch-impl: a top-level `{! ...}` block needs an attached type \
                 (the spec body is prepended to the macro input)"
            } else {
                "batch-impl: a bare `{...}` block without an attached type \
                 generates no impl (attach it to a type, e.g. `T { ... }`, or \
                 use the top-level `{! ...}` macro form)"
            },
            code.0
                .clone()
                .into_iter()
                .next()
                .map_or_else(proc_macro2::Span::call_site, |t| t.span()),
        );
    }
    // Top-level macro form: a chain ending in a `{! ...}` block (or the
    // `#cmd(args){body}` open-extension product) marks a macro call for
    // top-level emission — the `!` is stripped, the spec body (target type
    // + preceding blocks, merged in chain order into one Brace group) is
    // prepended to the macro input, and the call is emitted at top level
    // (no impl generated). The `{!}` block must be the last block.
    if let Some(result) = top_level_macro(&ty) {
        return match result {
            Ok((spec, mac)) => {
                if spec.is_empty() {
                    compile_error_str(
                        "batch-impl: a top-level `{! ...}` block needs an attached type \
                         (the spec body is prepended to the macro input)",
                        proc_macro2::Span::call_site(),
                    )
                } else if mac.is_empty() {
                    compile_error_str(
                        "batch-impl: a `{! ...}` top-level block must contain a macro \
                         call (e.g. `{! my_macro!{...}}`)",
                        proc_macro2::Span::call_site(),
                    )
                } else {
                    finalize_fresh_names(rewrite_macro_input(mac, spec))
                }
            }
            Err(e) => e,
        };
    }
    if let Ty { kind: TyKind::Error(e), .. } = ty {
        return e.0;
    }
    let parts = extract_impl_parts(ty);

    // Bound-generator distribution: a generator **range** inside an
    // impl-generic bound (`<T: Fn.().0..4 R>`) expands to a `TyArray` at the
    // apply layer; each element becomes its own impl with the bound pinned to
    // that arity (the array never renders inside a predicate). Runs before
    // every other generics concern so the distributed impls flow through the
    // pipeline independently (fresh hoisting, `@0..` re-opening, sweeping).
    let mut out = TokenStream::new();
    for parts in bound_gen::distribute_bound_arrays(parts) {
        out.extend(generate_parts(
            parts,
            trait_name,
            is_unsafe_trait,
            trait_bounds,
            trait_param_names,
        ));
    }
    out
}
