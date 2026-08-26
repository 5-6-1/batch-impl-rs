//! The post-extraction pipeline: one ImplParts -> one rendered impl block.
//! Split from mod.rs so the entry stays thin; the *order of application*
//! documented here is the single authority for the codegen stages.

use proc_macro2::TokenStream;
use std::cell::Cell;
use std::collections::HashSet;

use crate::ast::*;
use crate::util::compile_error_str;

use super::*;

/// The post-extraction pipeline: one ImplParts → one rendered impl block
/// (generics concerns → sync → where → shape → render). Split out of
/// [generate_impl] so bound-generator distribution can run each element
/// through the full pipeline independently.
#[allow(clippy::too_many_arguments)]
pub(crate) fn generate_parts(
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

    // Same-name declaration merge: chained `<>` blocks (`<T: Clone><T: Copy> X`)
    // would declare `T` twice (invalid Rust). Keep a single bare declaration and
    // move every bound of that name into a where predicate
    // (`impl<T> ... where T: Clone, T: Copy`); single declarations are untouched.
    crate::codegen::generics::merge_dup_params(&mut parts);

    // Impl generic names, normalized for const params (`const N` in the parse
    // layer — the keyword is needed to render `const N: usize`; bare `N` here
    // to match trait args and where-predicate refs). Shared by bound
    // inheritance and where-predicate resolution. Fresh declarations are
    // still their identity carriers at this point.
    let impl_name_streams = parts
        .impl_generics
        .iter()
        .map(|(n, _)| crate::codegen::generics::bare_param_name(n))
        .collect::<Vec<TokenStream>>();

    // The collision set display names must skip: every ident the impl
    // already writes (user params, target type, trait args, predicates,
    // body, attrs, associated types). Template placeholders are excluded —
    // the shape mapping rewrites them away before output, so counting them
    // would shift numbering the rendered impl never shows.
    let mut used = HashSet::new();
    collect_used_idents(&parts.target_type.to_token_stream(), &mut used);
    for ts in &parts.trait_generic_names {
        collect_used_idents(ts, &mut used);
    }
    for p in &parts.where_clauses {
        collect_used_idents(p, &mut used);
    }
    if let Some(b) = &parts.body {
        collect_used_idents(b, &mut used);
    }
    for a in &parts.attrs {
        collect_used_idents(a, &mut used);
    }
    for (_, v) in &parts.associated_types {
        collect_used_idents(v, &mut used);
    }
    for n in &impl_name_streams {
        if crate::ast::fresh::decl_fresh_pos(n).is_none() {
            collect_used_idents(n, &mut used);
        }
    }

    // The per-impl macro-meta context: fresh declarations sorted by
    // (group, position), each assigned its final display name — shared by
    // every `@` consumer from here on (inheritance / sync / where resolution
    // / range re-opening / repeat drivers / render).
    let fresh_ctx = FreshCtx::new(&impl_name_streams, &used);
    let trait_args = parts.trait_generic_names.clone();

    let mut errs = vec![];
    inherit_trait_bounds(&mut parts, trait_bounds, &trait_args);
    // `X<>` (empty angle brackets) → `X<spec args>` — where predicates,
    // `impl{...}` templates and impl-generic bounds fill unconditionally; a
    // **switch template** (`impl{@trait<>}` / `impl{Tr<>}`) additionally
    // turns on **body** sync (see `sync.rs`).
    if let Err(e) = sync_impl_parts(&mut parts, trait_name) {
        return e;
    }
    // where-predicate macro-meta replacement (`@N` → impl generic N) + bare-splat rejection
    let where_resolved = match resolve_where_predicates(&parts.where_clauses, &fresh_ctx) {
        Ok(ws) => ws,
        Err(es) => {
            errs.extend(es);
            vec![]
        }
    };
    // `@N` / `@g_i` in the target type / trait args (where predicates are
    // validated by resolve_where_predicates): a dangling reference would leak
    // an internal carrier into rustc's E0412 output — validate here and
    // report in user language. Runs before the declarations are renamed so
    // the declared set is still readable off the carriers.
    errs.extend(validate_at_refs(&parts.target_type, &parts.trait_generic_names, &fresh_ctx));
    if !errs.is_empty() {
        return errs.into_iter().collect();
    }

    // Declarations take their final display names — after validation, before
    // anything renders; no internal carrier ever reaches the output.
    rename_fresh_decls(&mut parts.impl_generics, &fresh_ctx);

    // Impl-generic **bounds** may hold fresh references (a bound generator's
    // params ride out of the bound but its references stay): resolve them to
    // display names like every other type position.
    for (_, bound) in &mut parts.impl_generics {
        if let Some(b) = bound
            && b.to_token_stream().to_string().contains('@')
        {
            let resolved = match crate::codegen::range_refs::expand_range_refs(
                b.to_token_stream(),
                &fresh_ctx,
            ) {
                Ok(t) => t,
                Err(e) => return e,
            };
            *b = TyPrimitive(resolved).to_ty();
        }
    }

    // `@0..` in the impl-generic declaration position (`<@0..>` declares every
    // fresh the range covers). Runs with the resolved context, so inserted
    // entries already carry display names — unique among freshs and skipping
    // every user-written ident by construction; an overlap with an existing
    // declaration is skipped, not duplicated.
    if let Err(e) =
        crate::codegen::range_refs::expand_range_decls(&mut parts.impl_generics, &fresh_ctx)
    {
        return e;
    }

    // The target type's references resolve now: its tokens must be valid
    // Rust before the shape kernel syn-parses them, and the resolvers emit
    // final names — nothing downstream renames idents anymore.
    let target_tokens = match crate::codegen::range_refs::expand_range_refs(
        parts.target_type.to_token_stream(),
        &fresh_ctx,
    ) {
        Ok(t) => t,
        Err(e) => return e,
    };
    // shape template: the `impl{...}` shape templates — match each template
    // against the leaf target type, merge the slot mappings, and apply the
    // rewrites (where predicates + body here; the target type at render,
    // where the final tokens are in hand). An empty template list is the
    // no-op case. Variadic segments (`ident@..`) additionally drive the
    // body's repeat blocks (`@(...)..`), which expand before the slot
    // mapping rewrites the resulting segment names.
    let (shape_map, var_segs) = if parts.impl_templates.is_empty() {
        (crate::codegen::Mapping::default(), Vec::new())
    } else {
        match crate::codegen::render::collect_shape_mapping(&target_tokens, &parts.impl_templates) {
            Ok((m, s)) => (m, s),
            Err(e) => return compile_error_str(&e.message(), proc_macro2::Span::call_site()),
        }
    };
    if !shape_map.slots().is_empty() {
        parts.where_clauses =
            parts.where_clauses.iter().map(|p| apply_mapping(p.clone(), &shape_map)).collect();
    }
    if let Some(b) = &mut parts.body {
        // Body token postprocessing lives together here, where the impl's
        // fresh names are in hand:
        // 1. Repeat blocks expand (`@(…@0,)..`; without an `impl{...}` the
        //    **fresh-binding switch** `impl{@0..}` drives cursor-only blocks
        //    — one round per bound fresh — and enables `@{N}` references).
        //    The expansion splices each segment round's bound element
        //    directly (the `$(...)*` semantics) — the shape mapping is the
        //    value source, no intermediate spelling reaches the body.
        // 2. Fresh-range placeholders re-open (`#map`-copied signatures
        //    substitute the trait's generic args verbatim, so `(@0..)`
        //    lands in the body as a carrier reference).
        //
        // A `@{...}` fresh-position carrier in the body is legal only with
        // a body-slot switch declared — `impl{@{}}`, or the fresh-binding
        // switch `impl{@N..}` whose rounds consume `@{N}` references — the
        // "declare what you use" rule. Without either, a carrier in the body
        // errors with guidance.
        if !parts.body_at && parts.fresh_binding.is_none() && crate::ast::fresh::body_has_carrier(b)
        {
            return compile_error_str(
                "batch-impl: a `@{N}` fresh reference in the body requires the \
                 `impl{@{}}` body-slot switch (declare it on the spec, e.g. \
                 `impl{@{}}`); without it, `@` in a body starts a repeat block",
                proc_macro2::Span::call_site(),
            );
        }
        let binding = parts.fresh_binding;
        let cx = RepeatCtx {
            segs: &var_segs,
            map: &shape_map,
            fresh: &fresh_ctx,
            binding,
            budget: Cell::new(MAX_REPEAT_TOKENS),
        };
        let expanded = match expand_repeat_blocks(b.clone(), &cx) {
            Ok(e) => e,
            Err(e) => return e,
        };
        let expanded = match crate::codegen::range_refs::expand_range_refs(expanded, &fresh_ctx) {
            Ok(e) => e,
            Err(e) => return e,
        };
        *b = if shape_map.slots().is_empty() {
            expanded
        } else {
            apply_mapping(expanded, &shape_map)
        };
    }
    crate::codegen::render::render_impl(
        parts,
        where_resolved,
        target_tokens,
        trait_name,
        is_unsafe_trait,
        &shape_map,
        &fresh_ctx,
    )
}
