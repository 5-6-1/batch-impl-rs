//! Codegen layer: impl block generation.
//!
//! Recursively dismantles each flattened leaf [`Ty`] (see `lib::parse_batch_trait_entry`)
//! into an [`ImplParts`] (impl generics, trait generics, associated type bindings,
//! target type, body, attrs, unsafe flag), then renders the final
//! `impl<...> Trait<...> for Target { ... }` block.

mod fresh;
mod impl_parts;
mod postprocess;
mod repeat;
mod shape;
mod sync_trait;
mod top_level;
mod where_at;

pub(crate) use fresh::*;
pub(crate) use impl_parts::*;
pub(crate) use postprocess::*;
pub(crate) use repeat::*;
pub(crate) use shape::*;
pub(crate) use sync_trait::*;
pub(crate) use top_level::*;
pub(crate) use where_at::*;

use crate::TraitBounds;
use crate::ast::types_render::render_param;
use crate::ast::*;
use crate::util::compile_error_str;
use proc_macro2::{Ident, TokenStream, TokenTree};
use quote::{ToTokens, quote};
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
    // bare code block: `{...}` as the whole spec → emit verbatim as a top-level item
    // (not wrapped in an impl block). A `!`-marked block (top-level macro form)
    // without an attached type has no spec body to prepend — error instead of
    // emitting invalid Rust (`!` is not an item).
    if let Ty { kind: TyKind::WithCode(TyWithCode(None, code)), .. } = &ty {
        let is_top_marked = matches!(
            code.0.clone().into_iter().next(),
            Some(TokenTree::Punct(p)) if p.as_char() == '!'
        );
        if is_top_marked {
            return compile_error_str(
                "batch-impl: a top-level `{! ...}` block needs an attached type \
                 (the spec body is prepended to the macro input)",
                code.0
                    .clone()
                    .into_iter()
                    .next()
                    .map_or_else(proc_macro2::Span::call_site, |t| t.span()),
            );
        }
        return code.0.clone();
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
    let mut parts = extract_impl_parts(ty);

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

    // Impl generic names, normalized for const params (`const N` in the parse
    // layer — the keyword is needed to render `const N: usize`; bare `N` here
    // to match trait args and where-predicate refs). Shared by bound
    // inheritance and where-predicate resolution.
    let impl_name_streams =
        parts.impl_generics.iter().map(|(n, _)| bare_param_name(n)).collect::<Vec<TokenStream>>();
    let impl_names = impl_name_streams.iter().map(|n| n.to_string()).collect::<HashSet<String>>();
    let trait_args =
        parts.trait_generic_names.iter().map(|n| n.to_string()).collect::<Vec<String>>();

    // inherit trait generic bounds: same-name inheritance vs. mismatch errors; see trait_bounds docs
    let mut errs = inherit_trait_bounds(&mut parts, trait_bounds, &trait_args, &impl_names);
    // `X<>` (same-named empty trait brackets) → the spec trait application:
    // where predicates, `impl{...}` templates and impl-generic bounds can
    // write `Semiring<>` instead of repeating `Semiring<Additive,
    // Multiplicative>`. The args come from the spec's parsed trait part — no
    // state. A `X<>` for any other trait errors; bounds sync on the Ty
    // structure (the DSL parse drops the empty brackets on render — see
    // `sync_bound_ty`).
    if let Some(trait_ident) = trait_last_ident(trait_name) {
        let trait_args = parts.trait_generic_names.clone();
        // Body sync is declared by a **switch template** (`impl{Tr<>}` /
        // `impl{@trait<>}` — the empty-bracket trait alone): unlike ordinary
        // `impl{...}` shape templates it does not match Self, it only syncs
        // `Tr<>` → `Tr<...>` in the template itself and turns on body sync
        // (the body is arbitrary Rust — a `Vec<>` there is not a trait
        // reference). Ordinary templates keep participating in the shape
        // match.
        let mut body_sync = false;
        let mut matched = Vec::new();
        for t in std::mem::take(&mut parts.impl_templates) {
            let is_switch =
                is_switch_template(&t.clone().into_iter().collect::<Vec<_>>(), &trait_ident);
            match sync_trait_application(t, &trait_ident, &trait_args) {
                Ok(s) => {
                    if is_switch {
                        body_sync = true;
                    } else {
                        matched.push(s);
                    }
                }
                Err(e) => return e,
            }
        }
        parts.impl_templates = matched;
        if body_sync && let Some(b) = &mut parts.body {
            *b = match sync_trait_application(b.clone(), &trait_ident, &trait_args) {
                Ok(s) => s,
                Err(e) => return e,
            };
        }
        let mut synced = Vec::with_capacity(parts.where_clauses.len());
        for w in &parts.where_clauses {
            match sync_trait_application(w.clone(), &trait_ident, &trait_args) {
                Ok(s) => synced.push(s),
                Err(e) => return e,
            }
        }
        parts.where_clauses = synced;
        // bounds: the empty brackets are lost in the Ty parse (render drops
        // them), so the sync works on the Ty structure — see `sync_bound_ty`.
        for (_, bound) in &mut parts.impl_generics {
            if let Some(b) = bound {
                *b = match sync_bound_ty(b, &trait_ident, &trait_args) {
                    Ok(t) => t,
                    Err(e) => return e,
                };
            }
        }
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
    // Ext 2: `impl{...}` Self-part shape templates — match each template
    // against the leaf target type, merge the slot mappings, and apply the
    // rewrites (where predicates + body here; the target type at render,
    // where the final tokens are in hand). An empty template list is the
    // no-op case. Variadic segments (`ident@..`) additionally drive the
    // body's repeat blocks (`@(...)..`), which expand before the slot
    // mapping rewrites the resulting segment names.
    let (shape_entries, var_segs) = if parts.impl_templates.is_empty() {
        (Vec::new(), Vec::new())
    } else {
        match collect_shape_mapping(&parts) {
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
    render_impl(parts, where_resolved, trait_name, is_unsafe_trait, &shape_entries)
}

/// Matches every `impl{...}` template against the leaf target type and
/// merges the slot mappings (identical re-bindings legal, conflicting ones
/// error). Both sides must be standard Rust types: the template is
/// user-written (syn-parsed, with variadic-segment placeholders already in
/// place), the target is the rendered leaf (a generator / DSL leftover
/// cannot be destructured). Returns the merged mapping and the resolved
/// variadic segments.
fn collect_shape_mapping(parts: &ImplParts) -> Result<(Mapping, Vec<VarSeg>), ShapeError> {
    let target_tokens = parts.target_type.to_token_stream();
    let target: syn::Type = syn::parse2(target_tokens).map_err(|_| {
        ShapeError::ShapeMismatch(
            "the target type is not a standard Rust type (DSL leftovers cannot be destructured by an `impl{...}` template)"
                .into(),
        )
    })?;
    let mut merged = Mapping::default();
    let mut segs = vec![];
    for t in &parts.impl_templates {
        let template: syn::Type = syn::parse2(t.clone()).map_err(|_| {
            ShapeError::ShapeMismatch(
                "the `impl{...}` template is not a standard Rust type (DSL operators are not allowed inside)"
                    .into(),
            )
        })?;
        let (m, s) = match_shape(&template, &target)?;
        merged.merge(m)?;
        segs.extend(s);
    }
    Ok((merged, segs))
}

/// Renders an impl generic name with the `const` keyword stripped (the parse
/// layer keeps `const` so `const N: usize` renders correctly; the bare name is
/// used for trait-arg matching and where-predicate references). Names are
/// always a single ident or the `const` ident pair; the fallback arm keeps the
/// token stream as-is so this helper can never panic (defensive — unreachable
/// in practice, kept to uphold the no-panic promise).
fn bare_param_name(name: &TokenStream) -> TokenStream {
    let mut tokens = name.clone().into_iter();
    match (tokens.next(), tokens.next()) {
        (Some(TokenTree::Ident(id)), None) => quote!(#id),
        (Some(TokenTree::Ident(kw)), Some(TokenTree::Ident(id)))
            if kw == "const" && tokens.next().is_none() =>
        {
            quote!(#id)
        }
        _ => name.clone(),
    }
}

/// Renders the final `impl<...> Trait<...> for Target where ... { ... }`
/// block from the extracted parts (bounds inherited, `@` refs resolved).
/// `shape_entries` (Ext 2 `impl{...}` slot mapping) rewrites the target
/// type at render — the where predicates and body were already rewritten by
/// the caller.
fn render_impl(
    parts: ImplParts, where_resolved: Vec<TokenStream>, trait_name: &TokenStream,
    is_unsafe_trait: bool, shape_entries: &[(String, TokenStream)],
) -> TokenStream {
    let is_unsafe = is_unsafe_trait || parts.is_unsafe_impl;
    let unsafe_kw = if is_unsafe { quote!(unsafe) } else { quote!() };

    // impl generic params (with bounds)
    let impl_gen = if parts.impl_generics.is_empty() {
        quote!()
    } else {
        let params = parts
            .impl_generics
            .iter()
            .map(|(name, bound)| render_param(name, bound.as_ref()))
            .collect::<Vec<_>>();
        quote!(<#(#params),*>)
    };

    // trait generic params (names only)
    let trait_gen = if parts.trait_generic_names.is_empty() {
        quote!()
    } else {
        let names = &parts.trait_generic_names;
        quote!(<#(#names),*>)
    };

    // target type — Ext 2 slot mapping applied at render (the leaf tokens
    // are in hand here; slot names in the target are replaced with the
    // bound subtrees, e.g. `A<B>` → `Box<usize>`).
    let target = if shape_entries.is_empty() {
        parts.target_type.to_token_stream()
    } else {
        apply_mapping(parts.target_type.to_token_stream(), shape_entries)
    };

    // impl body: associated types + user body
    let mut body_tokens: Vec<TokenStream> =
        parts.associated_types.iter().map(|(name, value)| quote!(type #name = #value;)).collect();
    if let Some(body) = &parts.body {
        body_tokens.push(body.clone());
    }

    // attributes
    let attrs = parts.attrs;

    // where clause: join predicates with commas; empty if no where (resolve_where_at already ran)
    let where_clause = if where_resolved.is_empty() {
        quote!()
    } else {
        let preds = &where_resolved;
        quote!(where #(#preds),*)
    };

    // Splat expansion happens structurally in `expand_splat_elems` (target /
    // trait args); bodies are never touched, so `a * b` inside a fn stays
    // multiplication. `where`-predicate splats are unsupported (rustc error).
    let rendered = quote! {
        #(#attrs)*
        #unsafe_kw impl #impl_gen #trait_name #trait_gen for #target #where_clause {
            #(#body_tokens)*
        }
    };
    sweep_fresh_names(rendered)
}
