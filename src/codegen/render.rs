//! Impl rendering and impl-generic postprocessing: the helpers of
//! `generate_impl` (in `mod.rs`) — shape-template slot-mapping collection,
//! same-name generic declaration merging, and the final `impl` block render.

use std::collections::{HashMap, HashSet};

use proc_macro2::{TokenStream, TokenTree};
use quote::{ToTokens, quote};

use crate::ast::Ty;
use crate::ast::types_render::render_param;
use crate::codegen::impl_parts::ImplParts;
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

/// Renders an impl generic name with the `const` keyword stripped (the parse
/// layer keeps `const` so `const N: usize` renders correctly; the bare name is
/// used for trait-arg matching and where-predicate references). Names are
/// always a single ident or the `const` ident pair; the fallback arm keeps the
/// token stream as-is so this helper can never panic (defensive — unreachable
/// in practice, kept to uphold the no-panic promise).
pub(crate) fn bare_param_name(name: &TokenStream) -> TokenStream {
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

/// Merges same-name impl generic declarations from chained `<>` blocks.
///
/// `<T: Clone><T: Copy> X` would render `impl<T: Clone, T: Copy>` — a
/// duplicate `T` declaration (E0415). Duplicate names collapse into one
/// **bare** declaration and every bound of that name moves into a where
/// predicate (`impl<T> ... where T: Clone, T: Copy`); the duplicate names
/// themselves are dropped. Names declared once are untouched (`<T: Clone>`
/// stays `impl<T: Clone>`). Const params (`const N: usize`) keep their full
/// declaration (the type annotation lives in the name tokens — there is
/// nowhere else for it to go; the later duplicates are simply dropped).
pub(crate) fn merge_dup_params(parts: &mut ImplParts) {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for (name, _) in &parts.impl_generics {
        *counts.entry(bare_param_name(name).to_string()).or_insert(0) += 1;
    }
    let mut merged: Vec<(TokenStream, Option<Ty>)> = Vec::new();
    let mut extra_where: Vec<TokenStream> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for (name, bound) in std::mem::take(&mut parts.impl_generics) {
        let name_str = name.to_string();
        let is_const = name_str.starts_with("const");
        let key = bare_param_name(&name).to_string();
        if counts.get(&key).copied().unwrap_or(0) > 1 {
            // duplicate name: bare single declaration (or the first full
            // const declaration), every bound moved into a where predicate
            if is_const {
                if !seen.insert(key) {
                    continue; // drop later const duplicates entirely
                }
                merged.push((name, bound));
            } else {
                if seen.insert(key.clone()) {
                    merged.push((name.clone(), None));
                }
                if let Some(b) = bound {
                    extra_where.push(quote!(#name: #b));
                }
            }
        } else {
            merged.push((name, bound));
        }
    }
    parts.impl_generics = merged;
    parts.where_clauses.extend(extra_where);
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
