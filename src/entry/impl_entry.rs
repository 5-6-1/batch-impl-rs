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
//! An **empty** spec list (`#[batch_impl]` / `#[batch_impl()]`, or a list of
//! separators only) is a **no-op**: nothing is derived, so the original block is
//! emitted unchanged. The attribute's contract is a flat-map over its specs, and
//! the identity of that map is the item itself — which is also the safe failure
//! mode, since a silently swallowed hand-written impl is invisible (measured:
//! without the guard, `#[batch_impl] impl Tr for u8 {…}` compiles with no
//! diagnostic *and* no `impl Tr for u8`).
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
use crate::ast::reset_fresh_counter;
use crate::codegen::{
    FreshCtx, Mapping, apply_mapping, collect_used_surfaces, expand_range_refs, hoist_type_params,
    match_shape, resolve_where_predicates, used_ident_set,
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
///
/// The attribute is a **derivation**: it is a flat-map over its `;`-separated
/// specs, each of which replaces the original block with the impls it derives
/// from it (0..N — a matrix leaf each). An **empty** spec list derives nothing,
/// so it is a no-op: the original block is emitted unchanged rather than
/// swallowed. That is the identity of the derivation, and it is also the safe
/// failure mode — a macro-generated or accidentally emptied attribute must not
/// delete a hand-written impl silently.
///
/// **Stacked `#[batch_impl]` attributes are stages of one derivation.** rustc
/// expands the outermost attribute first and hands it the rest in `item.attrs`;
/// this entry re-emits them on the impls it derives, so the compiler then expands
/// the next stage *on those impls*, and so on. The stages therefore run in
/// **source order** (top → bottom) over the accumulating block: a slot an earlier
/// stage leaves in place is bound by a later one, which is what makes a *shape
/// family* (containers that are not the same head) expressible as two stages —
/// see `tests/features/impl_entry_chain.rs`, which pins the order, the product,
/// the empty stage and the per-level attributes. Each stage is consumed where it
/// stands, so the impls that finally reach the compiler carry no `#[batch_impl]`
/// left, and an attribute between two stages belongs to the **expansion level**
/// it is written at, which is also what scopes a `#[cfg]` there. The order is the
/// compiler's, not this entry's state — the tests are the contract for it.
pub(crate) fn expand_impl_entry(
    attr: TokenStream, item: ItemImpl,
) -> Result<TokenStream, TokenStream> {
    let trait_path = item.trait_.as_ref().map(|(path, _)| path.clone());

    let attr_vec = attr.into_iter().collect::<Vec<_>>();
    // Nothing to derive: hand the item straight back (its own attributes
    // included — they ride on `item`, the `#[batch_impl(…)]` itself is consumed
    // by rustc). Separators are not content, so `#[batch_impl(;)]` counts as
    // empty too.
    if split_at_depth0(&attr_vec, ';').iter().all(|spec| spec.is_empty()) {
        return Ok(quote!(#item));
    }

    // ---- preprocessing subset (typestate pipeline, see
    // `preprocess/stream.rs`): bare `impl` collection → variadic-segment
    // marking → `@` constant expansion (built-in families + `@trait` → the
    // impl's own trait path; `@N` refs resolve against hoisted freshs) →
    // angle pairing → directive rejection (`#` banned on this entry) →
    // bare-`where` rewrite. The stream's states enforce the order; the
    // ItemImpl tail is `Paired → DirectivesResolved → WhereDone` ----
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
        // Fresh-generator group ids are DSL-local per spec (each spec
        // generates independent impl sets) — the same reset `batch_trait!`
        // performs per segment.
        reset_fresh_counter();
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
    let template_raw = crate::util::slice_upto(spec, colon)
        .iter()
        .cloned()
        .collect::<TokenStream>()
        .into_iter()
        .collect::<Vec<_>>();
    let template_marked = crate::preprocess::varseg::mark_template(&template_raw, 0)?;
    let template_tokens = render_angles(template_marked.into_iter().collect::<TokenStream>());
    let template: syn::Type = syn::parse2(template_tokens).map_err(|e| {
        compile_error_str(
            &format!("batch-impl: the shape template before `:` is not a valid type ({e})",),
            e.span(),
        )
    })?;
    let (new_gen, matrix) = split_new_gen(crate::util::slice_from(spec, colon + 1));
    // `used`: fresh display names must not collide with anything the impl
    // writes — the item (generics / body / where / associated types), the
    // template slots, the new generic decl and the spec's where predicates.
    // One source list, shared with the attribute entry's collision set
    // (`codegen::fresh_naming::used_ident_set`); the per-leaf extension (the matrix
    // source is user text too) joins in `expand_leaf` / `expand_direct_form`.
    let item_ts = item.to_token_stream();
    let template_ts = template.to_token_stream();
    let ng_ts = new_gen.as_ref().map(|ng| ng.to_token_stream());
    let where_ts = where_preds.iter().cloned().collect::<TokenStream>();
    let mut surfaces = vec![&item_ts, &template_ts, &where_ts];
    if let Some(ng) = &ng_ts {
        surfaces.push(ng);
    }
    let used = used_ident_set(&surfaces);
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
    // collecting its own shape templates (`[Box,Rc]impl{A<(T@..)>}` — several
    // may be comma-joined inside one `impl{...}`, like the attribute entry's
    // multi-template merge) and its where predicates.
    let mut leaf_templates: Vec<TokenStream> = vec![];
    let mut leaf_preds: Vec<TokenTree> = vec![];
    let mut leaf = Some(leaf);
    while let Some(t) = leaf {
        match t.kind {
            TyKind::WithImpl(wi) => {
                // An `impl{...}` attachment may hold several comma-separated
                // templates (`impl{A<B>, C<D>}`) — split and collect them
                // all; each matches the leaf and merges (the attribute
                // entry's `split_impl_attachments` is the shared splitter).
                leaf_templates.extend(crate::codegen::split_impl_attachments(&wi.1.0));
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
    // The leaf's own idents join the collision set: the matrix source is the
    // user's text (`Holder<P0>`), so a display name must never shadow it.
    let mut used = used.clone();
    let leaf_ts = leaf.to_token_stream();
    collect_used_surfaces(&[&leaf_ts], &mut used);
    let fresh_ctx = FreshCtx::new(&decl_names, &used);
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
    // The leaf's own templates (`impl{...}`) match the same leaf and merge:
    // their slots must agree (inconsistent bindings error), their segments
    // (the `T@..` driving the body's `fresh!`) join. The same merge the
    // attribute entry performs over multiple `impl{...}` attachments.
    for lt in leaf_templates {
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
    let item_ts = item.to_token_stream();
    let ng_ts = new_gen.as_ref().map(|ng| ng.to_token_stream());
    let where_ts = where_preds.iter().cloned().collect::<TokenStream>();
    let mut surfaces = vec![&item_ts, &where_ts];
    if let Some(ng) = &ng_ts {
        surfaces.push(ng);
    }
    let mut used = used_ident_set(&surfaces);
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
    // len == 1 verified above; the `if let` keeps the no-panic promise
    // instead of an (unreachable) unwrap.
    let Some(leaf) = leaves.into_iter().next() else {
        return Err(compile_error_str(
            "batch-impl: the direct form takes exactly one type after \
             the generic declaration (e.g. `<T> Box<T>`)",
            Span::call_site(),
        ));
    };
    let leaf = hoist_type_params(leaf, &mut fresh_decls);
    let decl_names = fresh_decls.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>();
    // The spec's for-type is user text too — its idents join the set.
    let leaf_ts = leaf.to_token_stream();
    collect_used_surfaces(&[&leaf_ts], &mut used);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Qualified types reach the generated impl. The spec's for-type is user
    /// text and `<T as Tr>::Assoc` is a projection, so three separate decisions
    /// must agree on the same `<>`-group discriminator (`split_projection`): the
    /// block parser must not read `<S as Tr>` as `Vec`'s argument list, the impl
    /// entry must not read it as the `new-generic-decl`, and neither may treat
    /// the `::` as the shape colon. Asserted on the rendered impls, because that
    /// is what the user sees.
    #[test]
    fn qualified_types_reach_the_generated_impl() {
        fn flat(ts: TokenStream) -> String {
            ts.to_string().chars().filter(|c| !c.is_whitespace()).collect()
        }
        let item: ItemImpl = syn::parse_quote!(impl M1 for Vec<u8> { fn one(&self) -> u8 { 1 } });
        let out = expand_impl_entry("Vec<<S as Tr>::Assoc>".parse().expect("spec parses"), item)
            .expect("the spec expands");
        assert_eq!(
            flat(out),
            flat(quote!(impl M1 for Vec<<S as Tr>::Assoc> { fn one(&self) -> u8 { 1 } })),
            "a projection as a generic argument"
        );
        let item: ItemImpl = syn::parse_quote!(impl M3 for u16 { fn three(&self) -> u8 { 3 } });
        let out = expand_impl_entry(
            "<<S as Tr>::Assoc as Tr>::Assoc".parse().expect("spec parses"),
            item,
        )
        .expect("the spec expands");
        assert_eq!(
            flat(out),
            flat(
                quote!(impl M3 for <<S as Tr>::Assoc as Tr>::Assoc { fn three(&self) -> u8 { 3 } })
            ),
            "a nested projection as the target"
        );
        let trait_item: syn::ItemTrait = syn::parse_quote!(
            trait M2 {
                fn two(&self) -> u8;
            }
        );
        let out = crate::entry::expand_attr_macro(
            "M2 <S as Tr>::Assoc { fn two(&self) -> u8 { 2 } }".parse().expect("spec parses"),
            trait_item,
            true,
        )
        .expect("the spec expands");
        assert_eq!(
            flat(out),
            flat(quote!(
                trait M2 {
                    fn two(&self) -> u8;
                }
                impl M2 for <S as Tr>::Assoc {
                    fn two(&self) -> u8 {
                        2
                    }
                }
            )),
            "a projection as the target, through the attribute entry"
        );
    }

    /// `R1` lock: the spec's own for-type is user text, so its idents must
    /// join the fresh collision set — the generator's display name has to
    /// escape (`P0A`) instead of shadowing a user type named `P0`, which is
    /// what the attribute entry already guaranteed. Before the source list
    /// was unified, the direct form collected only the item + new-gen and
    /// emitted `impl<P0> Tr for Holder<P0, (P0,)>` (a silently different
    /// impl: the generic shadows the user's `P0`).
    #[test]
    fn spec_idents_join_the_fresh_collision_set() {
        let attr: TokenStream = "Holder<P0, ()1>".parse().expect("the spec parses");
        let item: ItemImpl = syn::parse_quote!(
            impl Tr for X {
                fn f(&self) -> u8 {
                    0
                }
            }
        );
        let out = expand_impl_entry(attr, item).expect("the spec expands").to_string();
        assert!(
            out.contains("P0A"),
            "the fresh display name must escape the spec's own `P0`:\n{out}"
        );
    }

    /// `R1` phase-3 (partial) lock: the impl entry must not swallow the impl
    /// block's own attributes. `assemble_impl` renders the impl from the parts
    /// it extracts and used to leave `item.attrs` behind, so `#[cfg]` /
    /// `#[allow]` / `#[doc]` on the item vanished silently — the generated impl
    /// then existed unconditionally. The attribute entry has always inherited
    /// its spec's attachments; this is the impl-entry half of that contract.
    #[test]
    fn item_attributes_reach_the_generated_impl() {
        let attr: TokenStream = "Holder<u8>".parse().expect("the spec parses");
        let item: ItemImpl = syn::parse_quote!(
            #[allow(dead_code)]
            #[doc = "kept"]
            impl Tr for X {
                fn f(&self) -> u8 {
                    0
                }
            }
        );
        let out = expand_impl_entry(attr, item).expect("the spec expands").to_string();
        assert!(out.contains("allow"), "the item's `#[allow]` must ride out:\n{out}");
        assert!(out.contains("kept"), "the item's `#[doc]` must ride out:\n{out}");
    }
}
