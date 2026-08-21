//! The output concern: shape-template slot-mapping collection and the final
//! `impl<...>` block render. Split by concern — order of application is
//! described in `mod.rs`.

use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

use crate::ast::types_render::render_param;
use crate::codegen::extract::ImplParts;
use crate::codegen::shape::{Mapping, ShapeError, VarSeg, match_shape};

/// Matches every `impl{...}` template against the leaf target type and
/// merges the slot mappings (identical re-bindings legal, conflicting ones
/// error). Both sides must be standard Rust types: the template is
/// user-written (syn-parsed, with variadic-segment placeholders already in
/// place), the target is the rendered leaf (a generator / DSL leftover
/// cannot be destructured). Returns the merged mapping and the resolved
/// variadic segments.
pub(crate) fn collect_shape_mapping(
    parts: &ImplParts,
) -> Result<(Mapping, Vec<VarSeg>), ShapeError> {
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

/// Renders the final `impl<...> Trait<...> for Target where ... { ... }`
/// block from the extracted parts (bounds inherited, `@` refs resolved).
/// `shape_entries` (the `impl{...}` shape-template slot mapping) rewrites the target
/// type at render — the where predicates and body were already rewritten by
/// the caller.
pub(crate) fn render_impl(
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

    // target type — shape template slot mapping applied at render (the leaf tokens
    // are in hand here; slot names in the target are replaced with the
    // bound subtrees, e.g. `A<B>` → `Box<usize>`).
    let target = if shape_entries.is_empty() {
        parts.target_type.to_token_stream()
    } else {
        crate::codegen::shape::apply_mapping(parts.target_type.to_token_stream(), shape_entries)
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
    crate::codegen::fresh::sweep_fresh_names(rendered)
}
