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
//! fresh-generic name sweeper. Tests live beside their concern (`repeat_tests`,
//! `where_at_tests`).

mod bound_gen;
mod extract;
mod fresh;
mod generics;
mod match_ty;
mod range_refs;
mod render;
mod repeat;
mod repeat_drivers;
#[cfg(test)]
mod repeat_tests;
mod shape;
mod splat;
mod sync;
mod top_level;
mod where_at;
#[cfg(test)]
mod where_at_tests;

pub(crate) use extract::*;
pub(crate) use fresh::*;
pub(crate) use generics::*;
pub(crate) use repeat::*;
pub(crate) use shape::*;
pub(crate) use splat::*;
pub(crate) use sync::*;
pub(crate) use top_level::*;
pub(crate) use where_at::*;

use crate::TraitBounds;
use crate::ast::*;
use crate::util::compile_error_str;
use proc_macro2::{Ident, TokenStream, TokenTree};
use std::collections::HashSet;

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
                    sweep_fresh_names(rewrite_macro_input(mac, spec))
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
    for parts in crate::codegen::bound_gen::distribute_bound_arrays(parts) {
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

/// The post-extraction pipeline: one `ImplParts` → one rendered impl block
/// (generics concerns → sync → where → shape → render). Split out of
/// [`generate_impl`] so bound-generator distribution can run each element
/// through the full pipeline independently.
#[allow(clippy::too_many_arguments)]
fn generate_parts(
    mut parts: ImplParts, trait_name: &TokenStream, is_unsafe_trait: bool,
    trait_bounds: &TraitBounds, trait_param_names: &[Ident],
) -> TokenStream {
    // Codegen postprocess: substitute trait generic params in the body
    // (`From<bool>`: `value: T` → `value: bool` — the directive-copied
    // signature and user code block). ImplParts carries the arg names.
    substitute_trait_generics(&mut parts, trait_param_names);

    // Tuple-level splat expansion (Ty structure): `(A, *(B,C))` → `(A,B,C)`,
    // with fresh declarations from `*().N` hoisted. Runs before hoisting so
    // the lifted decl feeds into the impl generics. Generic-arg splats
    // (`T<*(A,B)>`) are structural (`TySplat` in `Box<Ty>` params) and expand
    // inside the same pass via `expand_tp`; trait-path splats (`Conv<*(A,B)>`)
    // expand in `extract_impl_parts` where the trait args are rendered.
    parts.target_type = expand_splat_elems(parts.target_type);

    // hoist nested `WithType` (fresh generics) out of the target type, preventing `<A>` leaks
    let mut nested_params = vec![];
    parts.target_type = hoist_type_params(parts.target_type, &mut nested_params);
    parts.impl_generics.extend(nested_params);

    // hoist fresh generics out of impl-generic **bounds** (`<T: Fn.().2>` →
    // the generator's `<P0,P1>` rides out of the bound, leaving `T: Fn(P0,P1)`;
    // the fresh declarations join the impl generics). A bound generator
    // (`Fn.().N`) declares its fresh params inside the bound Ty — they must
    // live on the impl, not inside the predicate.
    crate::codegen::generics::hoist_bound_fresh(&mut parts.impl_generics);

    // `@0..` in the impl-generic declaration position (`<@0..>` declares every
    // fresh the range covers). The fresh list is whatever the spec's
    // generators already declared (`*().N` / `().N`); a range with no fresh
    // coverage errors. Runs before `merge_dup_params` so overlapping
    // declarations collapse cleanly.
    {
        let names = parts
            .impl_generics
            .iter()
            .map(|(n, _)| crate::codegen::generics::bare_param_name(n))
            .collect::<Vec<_>>();
        if let Err(e) =
            crate::codegen::range_refs::expand_range_decls(&mut parts.impl_generics, &names)
        {
            return e;
        }
    }

    // Same-name declaration merge: chained `<>` blocks (`<T: Clone><T: Copy> X`)
    // would declare `T` twice (invalid Rust). Keep a single bare declaration and
    // move every bound of that name into a where predicate
    // (`impl<T> ... where T: Clone, T: Copy`); single declarations are untouched.
    crate::codegen::generics::merge_dup_params(&mut parts);

    // Impl generic names, normalized for const params (`const N` in the parse
    // layer — the keyword is needed to render `const N: usize`; bare `N` here
    // to match trait args and where-predicate refs). Shared by bound
    // inheritance and where-predicate resolution.
    let impl_name_streams = parts
        .impl_generics
        .iter()
        .map(|(n, _)| crate::codegen::generics::bare_param_name(n))
        .collect::<Vec<TokenStream>>();
    let impl_names = impl_name_streams.iter().map(|n| n.to_string()).collect::<HashSet<String>>();
    let trait_args =
        parts.trait_generic_names.iter().map(|n| n.to_string()).collect::<Vec<String>>();

    // inherit trait generic bounds: same-name inheritance vs. mismatch errors; see trait_bounds docs
    let mut errs = inherit_trait_bounds(&mut parts, trait_bounds, &trait_args, &impl_names);
    // `X<>` (empty angle brackets) → `X<spec args>` — where predicates,
    // `impl{...}` templates and impl-generic bounds fill unconditionally; a
    // **switch template** (`impl{@trait<>}` / `impl{Tr<>}`) additionally
    // turns on **body** sync (see `sync.rs`).
    if let Err(e) = sync_impl_parts(&mut parts, trait_name) {
        return e;
    }
    // where-predicate macro-meta replacement (`@N` → impl generic N) + bare-splat rejection
    let where_resolved = match resolve_where_predicates(&parts.where_clauses, &impl_name_streams) {
        Ok(ws) => ws,
        Err(es) => {
            errs.extend(es);
            vec![]
        }
    };
    // `@N` / `@g_i` in the target type / trait args (where predicates are
    // validated by resolve_where_predicates): a dangling reference would leak
    // the reserved `_Param_*_BatchGen_` name into rustc's E0412 output —
    // validate here and report in user language.
    errs.extend(validate_at_refs(
        &parts.target_type,
        &parts.trait_generic_names,
        &impl_name_streams,
    ));
    if !errs.is_empty() {
        return errs.into_iter().collect();
    }
    // shape template: the `impl{...}` shape templates — match each template
    // against the leaf target type, merge the slot mappings, and apply the
    // rewrites (where predicates + body here; the target type at render,
    // where the final tokens are in hand). An empty template list is the
    // no-op case. Variadic segments (`ident@..`) additionally drive the
    // body's repeat blocks (`@(...)..`), which expand before the slot
    // mapping rewrites the resulting segment names.
    let (shape_entries, var_segs) = if parts.impl_templates.is_empty() {
        (Vec::new(), Vec::new())
    } else {
        match crate::codegen::render::collect_shape_mapping(&parts) {
            Ok((m, s)) => (m.entries().to_vec(), s),
            Err(e) => return compile_error_str(&e.message(), proc_macro2::Span::call_site()),
        }
    };
    if !shape_entries.is_empty() {
        parts.where_clauses =
            parts.where_clauses.iter().map(|p| apply_mapping(p.clone(), &shape_entries)).collect();
        if let Some(b) = &mut parts.body {
            match expand_repeat_blocks(b.clone(), &var_segs) {
                Ok(expanded) => *b = apply_mapping(expanded, &shape_entries),
                Err(e) => return e,
            }
        }
    }
    crate::codegen::render::render_impl(
        parts,
        where_resolved,
        trait_name,
        is_unsafe_trait,
        &shape_entries,
        &impl_name_streams,
    )
}
