//! The impl entry of `#[batch_impl]` — batch-instantiate a
//! hand-written `impl` block from a shape-template × matrix-source
//! description.
//!
//! `#[batch_impl(A<B> : [Box,Rc].[usize,isize])] impl Tr for A<B> {...}`
//! emits one impl per matrix leaf (`Box<usize>` / `Box<isize>` / `Rc<usize>` /
//! `Rc<isize>`): the shape template is matched against each leaf by the
//! shared `codegen::shape` kernel, and the slot mapping rewrites the
//! for-Type / where predicates / body. The original impl is withheld (its
//! for-Type holds the placeholder slot names).
//!
//! The impl block itself is **ordinary Rust** (`impl Tr<...> for T { ... }`
//! must parse as `syn::ItemImpl` verbatim) — the DSL lives only in the
//! attribute. The body / for-Type therefore stay standard Rust: no
//! variadic segments, no repeat blocks, no DSL operators. `X<>` (empty
//! brackets) in the **where predicates** fills with the impl's trait args
//! (`impl Tr<Additive, Multiplicative> for ...` → `Marker<>` =
//! `Marker<Additive, Multiplicative>`), the same sync as the trait entries.
//!
//! Attr grammar (single-spec common case; `;` separates multiple specs):
//! - shape form: `shape-template : new-generic-decl? matrix-source? (where ...)?`
//! - direct form: `new-generic-decl? for-type (where ...)?`
//!
//! `@trait` (→ the impl's trait path) and the built-in `@` constants work on
//! this entry; generators in the target (`A<()0..=12>`) hoist their fresh
//! generics onto the impl and `@N..` where selectors resolve against them —
//! the spec layer shares the attribute entry's DSL. The impl block's
//! **body** stays ordinary Rust (no `@` carriers); `#` directives and `@N`
//! refs without a generator are rejected.

use proc_macro2::{Group, Span, TokenStream, TokenTree};
use quote::{ToTokens, quote};
use std::collections::HashSet;
use syn::ItemImpl;

use crate::codegen::{
    FreshCtx, Mapping, apply_mapping, collect_used_idents, expand_range_refs, hoist_type_params,
    match_shape, resolve_where_predicates,
};
use crate::entry::impl_spec::{
    assemble_impl, find_shape_colon, parse_matrix_leaves, peel_where, split_new_gen,
};
use crate::parse::split_at_depth0;
use crate::preprocess::{angle_collect, impl_process, render_angles, where_process};
use crate::util::compile_error_str;

/// Entry: expand `#[batch_impl(<dsl>)] impl ...` into N `impl` blocks.
/// Accepts both trait impls (`impl Trait for Type`) and **inherent impls**
/// (`impl Type` — same spec grammar, no `for` section rendered, `@trait`
/// banned).
pub(crate) fn expand_impl_entry(
    attr: TokenStream, item: ItemImpl,
) -> Result<TokenStream, TokenStream> {
    let trait_path = item.trait_.as_ref().map(|(path, _)| path.clone());

    // ---- preprocessing subset: `@` constant expansion (built-in families +
    // `@trait` → the impl's own trait path; `@N` refs and the `@all` selectors
    // are rejected — this entry has no fresh system / trait definition) →
    // angle pairing → directive rejection → bare-`where` rewrite (see the
    // entry module docs) ----
    let attr_vec = attr.into_iter().collect::<Vec<_>>();
    let trait_path_ts = trait_path.as_ref().map(|p| p.to_token_stream());
    let expanded = crate::preprocess::expand_consts(
        &attr_vec,
        crate::preprocess::ConstCtx::ItemImpl { trait_path: trait_path_ts.as_ref() },
    )?;
    // Entry conversion: flatten None groups + pair `<...>` (see angle_collect)
    let paired = angle_collect(&expanded)?;
    let paired = reject_directives(&paired)?;
    // The ItemImpl attr has no body after the predicates, so the end of the
    // stream terminates the where region (the predicates become a body-less
    // `where{...}` suffix).
    let paired = where_process(&paired)?;
    let paired = impl_process(&paired)?;

    // ---- `;`-separated specs (the single-spec case is the common one) ----
    let mut out = quote![];
    for spec in split_at_depth0(&paired, ';') {
        if spec.is_empty() {
            continue;
        }
        out.extend(expand_one_spec(spec, &item, trait_path.as_ref())?);
    }
    Ok(render_angles(out))
}

/// Expands one spec (shape form or direct form) into its impl(s).
fn expand_one_spec(
    spec: &[TokenTree], item: &ItemImpl, trait_path: Option<&syn::Path>,
) -> Result<TokenStream, TokenStream> {
    // `where{...}` (where_process output) is the tail.
    let (spec, where_preds) = peel_where(spec);
    match find_shape_colon(spec) {
        Some(colon) => {
            // ---- shape form: `shape-template : new-generic-decl? matrix-source?` ----
            // The angle groups must be restored to flat `<...>` before syn
            // parsing (render_angles; syn cannot consume the
            // `delimiter![<>]` carrier groups).
            let template_tokens =
                render_angles(spec[..colon].iter().cloned().collect::<TokenStream>());
            let template =
                syn::parse2(template_tokens).map_err(|e| {
                    compile_error_str(
                        &format!(
                            "batch-impl: the shape template before `:` is not a valid type ({e})",
                        ),
                        Span::call_site(),
                    )
                })?;
            // Shape-validity check: the impl's for-Type must
            // match the template ident-for-ident (zero bindings) — a binding
            // means the for-Type doesn't carry the placeholder slot names.
            let for_type = syn::parse2(item.self_ty.to_token_stream()).map_err(|_| {
                compile_error_str(
                    "batch-impl: the impl's for-Type is not a valid type",
                    Span::call_site(),
                )
            })?;
            let check = match_shape(&template, &for_type)
                .map(|(m, _)| m)
                .map_err(|e| compile_error_str(&e.message(), Span::call_site()))?;
            if !check.slots().is_empty() {
                return Err(compile_error_str(
                    "batch-impl: the impl's for-Type must match the shape template \
                     ident-for-ident (write the same placeholder names, e.g. `impl Tr for A<B>` \
                     with template `A<B>`)",
                    Span::call_site(),
                ));
            }
            let (new_gen, matrix) = split_new_gen(&spec[colon + 1..]);
            // `used`: fresh display names must not collide with anything the
            // impl writes (template slots, the new generic decl, the item).
            let mut used = HashSet::new();
            collect_used_idents(&item.to_token_stream(), &mut used);
            collect_used_idents(&template.to_token_stream(), &mut used);
            if let Some(ng) = &new_gen {
                collect_used_idents(&ng.to_token_stream(), &mut used);
            }
            // where predicates, split at depth-0 commas (each resolves
            // independently against the leaf's fresh names)
            let where_chunks = split_at_depth0(&where_preds, ',')
                .iter()
                .map(|c| c.iter().cloned().collect::<TokenStream>())
                .collect::<Vec<_>>();
            if matrix.is_empty() {
                // Empty matrix source → N = 1, the shape itself (no slot
                // mapping; the for-Type is emitted verbatim).
                let fresh_ctx = FreshCtx::new(&[], &used);
                let where_resolved = resolve_where_predicates(&where_chunks, &fresh_ctx)
                    .map_err(|es| es.into_iter().collect::<TokenStream>())?;
                return assemble_impl(
                    item,
                    trait_path,
                    new_gen.as_ref(),
                    &[],
                    &where_resolved,
                    &Mapping::default(),
                    item.self_ty.to_token_stream(),
                );
            }
            let leaves = parse_matrix_leaves(&matrix)?;
            let mut out = quote![];
            for leaf in leaves {
                // Generators in a leaf (`A<()0..=12>`) mint fresh
                // declarations: hoist them out of the leaf (they join the
                // impl generics), name them (`P0, P1, ...`) and resolve the
                // carriers to display names before the shape kernel
                // syn-parses the leaf.
                let mut fresh_decls = vec![];
                let leaf = hoist_type_params(leaf, &mut fresh_decls);
                let decl_names = fresh_decls.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>();
                let fresh_ctx = FreshCtx::new(&decl_names, &used);
                let fresh_names =
                    fresh_ctx.names.iter().map(|(_, _, n)| n.clone()).collect::<Vec<_>>();
                let leaf_tokens = expand_range_refs(leaf.to_token_stream(), &fresh_ctx)?;
                let leaf_ty = syn::parse2(leaf_tokens).map_err(|_| {
                    compile_error_str(
                        "batch-impl: the matrix leaf is not a standard Rust type \
                         (a generator's fresh generics could not be resolved)",
                        Span::call_site(),
                    )
                })?;
                let m = match_shape(&template, &leaf_ty)
                    .map(|(m, _)| m)
                    .map_err(|e| compile_error_str(&e.message(), Span::call_site()))?;
                // for-Type: slot names rewritten to the bound leaf subtrees.
                let for_ty = apply_mapping(item.self_ty.to_token_stream(), &m);
                let where_resolved = resolve_where_predicates(&where_chunks, &fresh_ctx)
                    .map_err(|es| es.into_iter().collect::<TokenStream>())?;
                out.extend(assemble_impl(
                    item,
                    trait_path,
                    new_gen.as_ref(),
                    &fresh_names,
                    &where_resolved,
                    &m,
                    for_ty,
                )?);
            }
            Ok(out)
        }
        None => {
            // ---- direct form: `new-generic-decl? for-type` (no matrix, N = 1) ----
            let (new_gen, for_tokens) = split_new_gen(spec);
            // The for-type is full DSL — a generator may appear; parse it
            // (the angle groups stay paired for the DSL parser), hoist the
            // freshs, name and resolve them to display names.
            let mut used = HashSet::new();
            collect_used_idents(&item.to_token_stream(), &mut used);
            if let Some(ng) = &new_gen {
                collect_used_idents(&ng.to_token_stream(), &mut used);
            }
            let where_chunks = split_at_depth0(&where_preds, ',')
                .iter()
                .map(|c| c.iter().cloned().collect::<TokenStream>())
                .collect::<Vec<_>>();
            let leaves = parse_matrix_leaves(&for_tokens.to_vec())?;
            if leaves.len() != 1 {
                return Err(compile_error_str(
                    "batch-impl: the direct form takes exactly one type after \
                     the generic declaration (e.g. `<T> Box<T>`)",
                    Span::call_site(),
                ));
            }
            let mut fresh_decls = vec![];
            let leaf = hoist_type_params(leaves.into_iter().next().unwrap(), &mut fresh_decls);
            let decl_names = fresh_decls.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>();
            let fresh_ctx = FreshCtx::new(&decl_names, &used);
            let fresh_names = fresh_ctx.names.iter().map(|(_, _, n)| n.clone()).collect::<Vec<_>>();
            let for_tokens = expand_range_refs(leaf.to_token_stream(), &fresh_ctx)?;
            let where_resolved = resolve_where_predicates(&where_chunks, &fresh_ctx)
                .map_err(|es| es.into_iter().collect::<TokenStream>())?;
            assemble_impl(
                item,
                trait_path,
                new_gen.as_ref(),
                &fresh_names,
                &where_resolved,
                &Mapping::default(),
                for_tokens,
            )
        }
    }
}

/// Rejects `#name(...)` directives (only `#[...]` attributes pass through) —
/// the ItemImpl entry has no directive system. `@` was handled earlier by
/// `expand_consts` (built-in constants + `@trait`).
fn reject_directives(tokens: &[TokenTree]) -> Result<Vec<TokenTree>, TokenStream> {
    let mut out = vec![];
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            // `#` directives are banned; `#[...]` attributes pass through.
            TokenTree::Punct(p) if p.as_char() == '#' => {
                if matches!(tokens.get(i + 1), Some(TokenTree::Group(g))
                    if g.delimiter() == delimiter![[]])
                {
                    out.push(tokens[i].clone());
                    out.push(tokens[i + 1].clone());
                    i += 2;
                } else {
                    return Err(compile_error_str(
                        "batch-impl: `#` directives are not supported on the ItemImpl entry \
                         (write the impl body directly)",
                        tokens[i].span(),
                    ));
                }
            }
            TokenTree::Group(g) => {
                let inner = reject_directives(&g.stream().into_iter().collect::<Vec<_>>())?;
                let mut ng = Group::new(g.delimiter(), inner.into_iter().collect());
                ng.set_span(g.span());
                out.push(TokenTree::Group(ng));
                i += 1;
            }
            _ => {
                out.push(tokens[i].clone());
                i += 1;
            }
        }
    }
    Ok(out)
}
