//! The output concern: shape-template slot-mapping collection and the final
//! `impl<...>` block render. Split by concern — order of application is
//! described in `mod.rs`.

use proc_macro2::TokenStream;
use quote::quote;

use crate::ast::types_render::render_param;
use crate::codegen::FreshCtx;
use crate::codegen::extract::ImplParts;
use crate::codegen::shape::{Mapping, ShapeError, VarSeg, match_shape};

/// Matches every `impl{...}` template against the leaf target type and
/// merges the slot mappings (identical re-bindings legal, conflicting ones
/// error). Both sides must be standard Rust types: the template is
/// user-written (syn-parsed, with variadic-segment placeholders already in
/// place), the target is the rendered leaf with its fresh references already
/// resolved to display names (a carrier is not valid Rust — syn could not
/// destructure it). Returns the merged mapping and the resolved variadic
/// segments.
pub(crate) fn collect_shape_mapping(
    target_tokens: &TokenStream, templates: &[TokenStream],
) -> Result<(Mapping, Vec<VarSeg>), ShapeError> {
    let target = syn::parse2(target_tokens.clone()).map_err(|_| {
        ShapeError::ShapeMismatch(
            "the target type is not a standard Rust type (DSL leftovers cannot be destructured by an `impl{...}` template)"
                .into(),
        )
    })?;
    let mut merged = Mapping::default();
    let mut segs = vec![];
    for t in templates {
        // The template's `<...>` was angle-paired by `angle_collect`
        // (`impl{...}` is now entered like `where{...}`), but syn needs flat
        // `<...>` — restore the pairing before parsing.
        let flat = crate::preprocess::render_angles(t.clone());
        let template = syn::parse2(flat).map_err(|_| {
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

/// Renders the final `impl<...> Trait<...> for Target where ... { ... }`
/// block from the extracted parts (bounds inherited, `@` refs resolved) — the
/// **one renderer** both front-ends go through. `target_tokens` is the target
/// type with its fresh references already resolved to display names (resolved in
/// `generate_parts`, before the shape kernel needs valid-Rust leaf tokens); the
/// shape-template slot mapping was applied to the where predicates and body by
/// the caller — the target gets the mapping here, where the final tokens are in
/// hand.
///
/// `trait_name` is `None` for an **inherent** impl (the `for` section is
/// omitted), which only the impl entry can produce. `fresh_ctx` is `None` when
/// there is nothing to re-open: it feeds the `@N..` range-placeholder expansion
/// of the trait args, and the impl entry's trait path is verbatim user Rust, so
/// it has no placeholders.
pub(crate) fn render_impl(
    parts: ImplParts, where_resolved: Vec<TokenStream>, target_tokens: TokenStream,
    trait_name: Option<&TokenStream>, is_unsafe_trait: bool, shape_map: &Mapping,
    fresh_ctx: Option<&FreshCtx>,
) -> TokenStream {
    let is_unsafe = is_unsafe_trait || parts.is_unsafe_impl;

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

    // trait generic params (names only) — `@N..` placeholders re-open here
    let mut trait_gen = quote!();
    if !parts.trait_generic_names.is_empty() {
        let Some(fresh_ctx) = fresh_ctx else {
            // Only the attribute entry has placeholders, and it always passes a
            // context; reaching this means the invariant broke, so report instead
            // of dropping the args silently.
            return crate::util::compile_error_str(
                "batch-impl: internal error: trait-arg placeholders without a fresh context \
                 (please report this spelling)",
                proc_macro2::Span::call_site(),
            );
        };
        let mut names = vec![];
        for n in &parts.trait_generic_names {
            match crate::codegen::range_refs::expand_range_refs(n.clone(), fresh_ctx) {
                Ok(expanded) => names.push(expanded),
                Err(e) => return e,
            }
        }
        trait_gen = quote!(<#(#names),*>);
    }

    // target type — shape template slot mapping applied here (the resolved
    // leaf tokens are in hand; slot names in the target are replaced with
    // the bound subtrees, e.g. `A<B>` → `Box<usize>`). References were
    // already resolved in `generate_parts`. Only the slots channel applies
    // — segment values splice into bodies during the repeat expansion and
    // never pass through the mapping.
    let target = if shape_map.slots().is_empty() {
        target_tokens
    } else {
        crate::codegen::shape::apply_mapping(target_tokens, shape_map)
    };

    // impl body: associated types + user body. Fresh-range placeholders in
    // the body were already re-opened by the codegen postprocess
    // (`expand_range_refs` in `generate_parts`, next to the repeat-block
    // expansion); render just assembles the tokens.
    let mut body_tokens = parts
        .associated_types
        .iter()
        .map(|(name, value)| quote!(type #name = #value;))
        .collect::<Vec<_>>();
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
    // Every internal name was resolved before this point: fresh declarations
    // carry their display names, references resolved against them — no
    // final renaming pass exists.
    let head = match trait_name {
        Some(t) => quote!(impl #impl_gen #t #trait_gen for #target),
        // Inherent impl (impl entry only): no `for` section.
        None => quote!(impl #impl_gen #target),
    };
    render_impl_block(&attrs, is_unsafe, head, where_clause, &body_tokens)
}

/// The impl block's **textual skeleton** — the one place that knows how a
/// generated impl is spelled: attributes, the `unsafe` keyword, the head, the
/// where clause and the body, in that order. `render_impl` (the attribute entry,
/// which types its inputs as `ImplParts`) and `assemble_impl` (the impl entry,
/// whose inputs stay token-level) both call it, so a rule about any of those
/// slots is written once instead of once per front-end — the attribute bug that
/// had to be fixed twice is what this exists to prevent.
///
/// `head` is the caller's business (`impl<…> Trait<…> for Target`, or the
/// inherent form without `for`), because that is exactly where the two entries'
/// input models differ.
pub(crate) fn render_impl_block(
    attrs: &[TokenStream], is_unsafe: bool, head: TokenStream, where_clause: TokenStream,
    body: &[TokenStream],
) -> TokenStream {
    let unsafe_kw = if is_unsafe { quote!(unsafe) } else { quote!() };
    quote! {
        #(#attrs)*
        #unsafe_kw #head #where_clause {
            #(#body)*
        }
    }
}
