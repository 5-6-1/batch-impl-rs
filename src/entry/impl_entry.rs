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

use proc_macro2::{Span, TokenStream, TokenTree};
use quote::{ToTokens, quote};
use std::collections::HashSet;
use syn::ItemImpl;

use crate::ast::TyKind;
use crate::codegen::{
    FreshCtx, Mapping, apply_mapping, collect_used_idents, expand_range_refs, hoist_type_params,
    match_shape, resolve_where_predicates,
};
use crate::entry::impl_spec::{
    assemble_impl, find_shape_colon, parse_matrix_leaves, peel_where, split_new_gen,
};
use crate::parse::split_at_depth0;
use crate::preprocess::consts::ConstCtx;
use crate::preprocess::render_angles;
use crate::preprocess::stream::new as stream_new;
use crate::util::compile_error_str;

/// Splits a token slice at depth-0 separators and renders each chunk back to
/// a `TokenStream` (the where-predicate splitting pattern shared by the shape
/// form, the leaf expansion and the direct form).
fn chunks_to_streams(tokens: &[TokenTree], sep: char) -> Vec<TokenStream> {
    split_at_depth0(tokens, sep)
        .iter()
        .map(|c| c.iter().cloned().collect::<TokenStream>())
        .collect()
}

/// Entry: expand `#[batch_impl(<dsl>)] impl ...` into N `impl` blocks.
/// Accepts both trait impls (`impl Trait for Type`) and **inherent impls**
/// (`impl Type` — same spec grammar, no `for` section rendered, `@trait`
/// banned).
pub(crate) fn expand_impl_entry(
    attr: TokenStream, item: ItemImpl,
) -> Result<TokenStream, TokenStream> {
    let trait_path = item.trait_.as_ref().map(|(path, _)| path.clone());

    // ---- preprocessing subset (typestate pipeline, see
    // `preprocess/stream.rs`): bare `impl` collection → variadic-segment
    // marking → `@` constant expansion (built-in families + `@trait` → the
    // impl's own trait path; `@N` refs resolve against hoisted freshs) →
    // angle pairing → directive rejection (`#` banned on this entry) →
    // bare-`where` rewrite. The stream's states enforce the order; the
    // ItemImpl tail is `Paired → DirectivesResolved → WhereDone` ----
    let attr_vec = attr.into_iter().collect::<Vec<_>>();
    let trait_path_ts = trait_path.as_ref().map(|p| p.to_token_stream());
    let paired = stream_new(attr_vec)
        .preprocess(ConstCtx::ItemImpl { trait_path: trait_path_ts.as_ref() })?
        .reject_directives()?
        .where_process()?;
    let paired = paired.into_tokens();

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
    // `where{...}` attachments of the template region are extracted at the
    // token level (the template must stay a syn type); the matrix region's
    // attachments ride the parse layer (`TyWithWhere`) and are extracted
    // per leaf.
    let (spec, where_preds) = peel_where(spec);
    match find_shape_colon(&spec) {
        Some(colon) => expand_shape_form(&spec, colon, &where_preds, item, trait_path),
        None => expand_direct_form(&spec, &where_preds, item, trait_path),
    }
}

/// Shape form: `shape-template : new-generic-decl? matrix-source?` — the
/// template matches each matrix leaf; the slot mapping is **textually
/// applied** to the impl's for-Type / where predicates / body (the for-Type
/// need not mirror the template ident-for-ident).
fn expand_shape_form(
    spec: &[TokenTree], colon: usize, where_preds: &[TokenTree], item: &ItemImpl,
    trait_path: Option<&syn::Path>,
) -> Result<TokenStream, TokenStream> {
    // The angle groups must be restored to flat `<...>` before syn parsing
    // (render_angles; syn cannot consume the `delimiter![<>]` carrier
    // groups). A template may declare variadic segments (`(T@..)` → the
    // `[T; ()]` marker) — matched against generator tuples by the shape
    // kernel.
    let template_raw =
        spec[..colon].iter().cloned().collect::<TokenStream>().into_iter().collect::<Vec<_>>();
    let template_marked = crate::preprocess::varseg::mark_template(&template_raw, 0)?;
    let template_tokens = render_angles(template_marked.into_iter().collect::<TokenStream>());
    let template: syn::Type = syn::parse2(template_tokens).map_err(|e| {
        compile_error_str(
            &format!("batch-impl: the shape template before `:` is not a valid type ({e})",),
            e.span(),
        )
    })?;
    let (new_gen, matrix) = split_new_gen(&spec[colon + 1..]);
    // `used`: fresh display names must not collide with anything the impl
    // writes (template slots, the new generic decl, the item).
    let mut used = HashSet::new();
    collect_used_idents(&item.to_token_stream(), &mut used);
    collect_used_idents(&template.to_token_stream(), &mut used);
    if let Some(ng) = &new_gen {
        collect_used_idents(&ng.to_token_stream(), &mut used);
    }
    if matrix.is_empty() {
        // Empty matrix source → N = 1, the shape itself (no slot mapping;
        // the for-Type is emitted verbatim).
        let where_chunks = chunks_to_streams(where_preds, ',');
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
            &[],
            item.self_ty.to_token_stream(),
        );
    }
    let leaves = parse_matrix_leaves(&matrix)?;
    let mut out = quote![];
    for leaf in leaves {
        out.extend(expand_leaf(
            leaf,
            &template,
            &used,
            where_preds,
            item,
            trait_path,
            new_gen.as_ref(),
        )?);
    }
    Ok(out)
}

/// Expands one matrix leaf: strips its attachments (borrowed from the parse
/// layer's block model — a `TyWithImpl` template pairing with its container,
/// `TyWithWhere` predicates), hoists generators' freshs, matches the shape
/// template(s), and assembles the impl.
fn expand_leaf(
    leaf: crate::ast::Ty, template: &syn::Type, used: &HashSet<String>, where_preds: &[TokenTree],
    item: &ItemImpl, trait_path: Option<&syn::Path>, new_gen: Option<&TokenStream>,
) -> Result<TokenStream, TokenStream> {
    // Attachment extraction: recursively strip the leaf's attachments,
    // collecting its own shape template (`[Box,Rc]impl{A<(T@..)>}`) and its
    // where predicates.
    let mut leaf_template = None;
    let mut leaf_preds: Vec<TokenTree> = vec![];
    let mut leaf = Some(leaf);
    while let Some(t) = leaf {
        match t.kind {
            TyKind::WithImpl(wi) => {
                leaf_template = Some(wi.1.0);
                leaf = wi.0.map(|b| *b);
            }
            TyKind::WithWhere(ww) => {
                if !leaf_preds.is_empty() {
                    leaf_preds.push(TokenTree::Punct(proc_macro2::Punct::new(
                        ',',
                        proc_macro2::Spacing::Alone,
                    )));
                }
                leaf_preds.extend(ww.1.0.clone().into_iter().collect::<Vec<_>>());
                leaf = wi_where_inner(ww.0);
            }
            _ => {
                leaf = Some(t);
                break;
            }
        }
    }
    let Some(leaf) = leaf else {
        return Err(compile_error_str(
            "batch-impl: an attachment (`impl{...}` / `where{...}`) in the matrix \
             source needs a container to pair with (e.g. `[Box,Rc] impl{A<(T@..)>}`)",
            Span::call_site(),
        ));
    };
    // Generators in a leaf (`A<()0..=12>`) mint fresh declarations: hoist
    // them out of the leaf (they join the impl generics), name them
    // (`P0, P1, ...`) and resolve the carriers to display names before the
    // shape kernel syn-parses the leaf.
    let mut fresh_decls = vec![];
    let leaf = hoist_type_params(leaf, &mut fresh_decls);
    let decl_names = fresh_decls.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>();
    let fresh_ctx = FreshCtx::new(&decl_names, used);
    let fresh_names = fresh_ctx.names.iter().map(|(_, _, n)| n.clone()).collect::<Vec<_>>();
    let leaf_tokens = expand_range_refs(leaf.to_token_stream(), &fresh_ctx)?;
    let leaf_span =
        leaf_tokens.clone().into_iter().next().map(|t| t.span()).unwrap_or_else(Span::call_site);
    let leaf_ty = syn::parse2(leaf_tokens).map_err(|e| {
        compile_error_str(
            "batch-impl: the matrix leaf is not a standard Rust type \
             (a generator's fresh generics could not be resolved)",
            e.span(),
        )
    })?;
    let (mut m, mut template_segs) =
        match_shape(template, &leaf_ty).map_err(|e| compile_error_str(&e.message(), leaf_span))?;
    // The leaf's own template (`impl{...}`) matches the same leaf and
    // merges: its slots must agree, its segments (the `T@..` driving the
    // body's `fresh!`) join.
    if let Some(lt) = leaf_template {
        let lt_marked =
            crate::preprocess::varseg::mark_template(&lt.into_iter().collect::<Vec<_>>(), 0)?;
        let lt_tokens = render_angles(lt_marked.into_iter().collect::<TokenStream>());
        let lt_span =
            lt_tokens.clone().into_iter().next().map(|t| t.span()).unwrap_or_else(Span::call_site);
        let lt_ty: syn::Type = syn::parse2(lt_tokens).map_err(|e| {
            compile_error_str("batch-impl: the `impl{...}` template is not a valid type", e.span())
        })?;
        let (m2, segs2) =
            match_shape(&lt_ty, &leaf_ty).map_err(|e| compile_error_str(&e.message(), lt_span))?;
        m.merge(m2).map_err(|e| compile_error_str(&e.message(), lt_span))?;
        template_segs.extend(segs2);
    }
    // for-Type: slot names rewritten to the bound leaf subtrees.
    let for_ty = apply_mapping(item.self_ty.to_token_stream(), &m);
    // where predicates: the template region's (peel_where) plus this leaf's
    // `where{...}` attachments — each resolves independently against the
    // leaf's fresh names.
    let mut chunks = chunks_to_streams(where_preds, ',');
    if !leaf_preds.is_empty() {
        chunks.extend(chunks_to_streams(&leaf_preds, ','));
    }
    let where_resolved = resolve_where_predicates(&chunks, &fresh_ctx)
        .map_err(|es| es.into_iter().collect::<TokenStream>())?;
    assemble_impl(
        item,
        trait_path,
        new_gen,
        &fresh_names,
        &where_resolved,
        &m,
        &template_segs,
        for_ty,
    )
}

/// The inner type of a `WithWhere` attachment (an `Option<Box<Ty>>`).
fn wi_where_inner(inner: Option<Box<crate::ast::Ty>>) -> Option<crate::ast::Ty> {
    inner.map(|b| *b)
}

/// Direct form: `new-generic-decl? for-type` (no matrix, N = 1) — the
/// for-type is full DSL (a generator may appear); hoist the freshs, name and
/// resolve them to display names.
fn expand_direct_form(
    spec: &[TokenTree], where_preds: &[TokenTree], item: &ItemImpl, trait_path: Option<&syn::Path>,
) -> Result<TokenStream, TokenStream> {
    let (new_gen, for_tokens) = split_new_gen(spec);
    let mut used = HashSet::new();
    collect_used_idents(&item.to_token_stream(), &mut used);
    if let Some(ng) = &new_gen {
        collect_used_idents(&ng.to_token_stream(), &mut used);
    }
    let where_chunks = chunks_to_streams(where_preds, ',');
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
        &[],
        for_tokens,
    )
}
