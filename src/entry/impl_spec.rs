//! Impl assembly and spec helpers for the impl entry (split into their own
//! file): `assemble_impl` renders one
//! generated impl from the extracted parts; the small helpers parse the
//! matrix source and split the shape-form spec.

use proc_macro2::{Group, TokenStream, TokenTree};
use quote::{ToTokens, quote};
use std::cell::Cell;
use syn::ItemImpl;

use crate::ast::{Op, Ty, TyPrimitive};
use crate::codegen::FreshCtx;
use crate::codegen::{
    ImplParts, MAX_REPEAT_TOKENS, Mapping, RepeatCtx, VarSeg, apply_mapping, apply_type_mapping,
    expand_repeat_blocks, sync_trait_application,
};
use crate::entry::driver::collect_spec_leaves;
use crate::util::{Cursor, compile_error_str, is_punct_at, is_single_colon, slice_from};

/// Builds the `ImplParts` for one generated impl and hands it to the shared
/// renderer (`codegen::render_impl`) — the same renderer the attribute entry uses,
/// so the impl block is spelled in exactly one place. What this function owns is
/// the entry's own input mapping:
///
/// - generics (attr new-generic-decl first, then the hoisted fresh names, then
///   the impl's own params — a param whose name is a shape-template **slot** is a
///   substitution target, not a declaration: the mapping already rewrote every
///   occurrence, so declaring it again would emit rustc E0207; it is stripped,
///   and its bounds become where predicates on the substituted type,
///   `impl<T: Clone>` → `where u8: Clone`);
/// - the trait path (**`None` for an inherent impl** — the renderer omits the
///   `for` section and the rewritten self type stands alone);
/// - the where predicates (synced for `X<>`, mapped) and the rewritten body;
/// - the item's own attributes (`#[cfg]` / `#[allow]` / `#[doc]`, …): the
///   attribute entry inherits the spec's attachments, and the impl entry used to
///   drop the item's silently — a `#[cfg]` there meant the generated impls
///   existed unconditionally.
///
/// `m` is the slot mapping (empty for the direct form / empty matrix).
// clippy's 7-argument threshold is not useful here: each parameter is one
// distinct input of the entry's mapping (the item, its trait path, the attr
// declaration, the hoisted fresh names, the where predicates, the shape mapping,
// the template segments, the mapped for-type), and grouping them into a struct
// would only move the list.
#[allow(clippy::too_many_arguments)]
pub(crate) fn assemble_impl(
    item: &ItemImpl, trait_path: Option<&syn::Path>, new_gen: Option<&TokenStream>,
    fresh_names: &[TokenStream], where_preds: &[TokenStream], m: &Mapping,
    template_segs: &[VarSeg], for_ty: TokenStream,
) -> Result<TokenStream, TokenStream> {
    let slot_names: std::collections::HashSet<&str> =
        m.slots().iter().map(|(n, _)| n.as_str()).collect();
    let mut item_params = vec![];
    let mut item_param_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut param_bound_preds: Vec<TokenStream> = vec![];
    for p in &item.generics.params {
        let name = crate::ast::name_of_generic_param(p);
        if slot_names.contains(name.as_str()) {
            // The slot mapping replaced every occurrence of the name (the
            // for-Type / where predicates / body are rewritten) — keeping
            // the declaration would be an unconstrained param (E0207). Its
            // bounds carry over as a where predicate on the substituted
            // type: `impl<T: Clone> Mk for Wrapper<T>` with leaf `u8` →
            // `impl Mk for Box<u8> where u8: Clone` (the name substitutes
            // through `apply_mapping` below).
            if let syn::GenericParam::Type(tp) = p
                && !tp.bounds.is_empty()
            {
                let ident = &tp.ident;
                let bounds = &tp.bounds;
                param_bound_preds.push(quote!(#ident: #bounds));
            }
            continue;
        }
        // The names are needed to reconcile the attr's own generic decl with
        // what the block already declares ([`reconcile_new_gen`]).
        item_param_names.insert(name);
        item_params.push(p.to_token_stream());
    }
    // Generics: the attr new-generic-decl first, then the hoisted fresh names
    // (`P0, P1, ...` from a generator in the target), then the impl's own
    // params. The attr's declaration is reconciled with the item's params (and
    // with the slot names) in **this one place** — one authority for "which
    // generics does the impl end up declaring", so neither entry form has to
    // re-derive it (F4 of the review pass: the direct form used to emit
    // `impl<T>` twice, `E0403`).
    let mut all_params = Vec::with_capacity(
        new_gen.map_or(0, |n| n.clone().into_iter().count())
            + fresh_names.len()
            + item_params.len(),
    );
    if let Some(ng) = new_gen {
        let (decl, dropped) = reconcile_new_gen(ng, &item_param_names, &slot_names);
        all_params.extend(decl);
        param_bound_preds.extend(dropped);
    }
    all_params.extend(fresh_names.iter().cloned());
    all_params.extend(item_params);
    // `X<>` sync: every `X<>` in the where predicates fills with the impl's
    // trait args (`impl Tr<Additive, Multiplicative> for ...` → `Marker<>` =
    // `Marker<Additive, Multiplicative>`). The body is not synced: it is
    // ordinary Rust (the impl block parses verbatim), so an empty bracket
    // there is a real Rust type, not a DSL trait reference. An inherent impl
    // has no trait args — sync degrades to a no-op.
    let trait_args = trait_path
        .and_then(|p| p.segments.last())
        .map(|seg| match &seg.arguments {
            syn::PathArguments::AngleBracketed(ab) => {
                ab.args.iter().map(|a| a.to_token_stream()).collect::<Vec<_>>()
            }
            _ => vec![],
        })
        .unwrap_or_default();
    // The ident the brackets are *allowed* to vanish on: only the annotated trait's own empty
    // `Tr<>` is legal (it has no arguments to copy). An inherent impl has no trait at all, so
    // `None` — and then every empty `<>` reports, which is the Ty-level rule.
    let trait_ident = trait_path.and_then(|p| p.segments.last()).map(|seg| seg.ident.clone());
    let mut preds = vec![];
    for p in where_preds {
        let p = sync_trait_application(p.clone(), &trait_args, trait_ident.as_ref())?;
        preds.push(apply_mapping(p, m));
    }
    if let Some(wc) = &item.generics.where_clause {
        let p = sync_trait_application(
            wc.predicates.to_token_stream(),
            &trait_args,
            trait_ident.as_ref(),
        )?;
        preds.push(apply_mapping(p, m));
    }
    // Bounds of stripped slot-named params (see the item-params loop above) —
    // synced like the other predicates (`X<>` fills with the trait args).
    for p in &param_bound_preds {
        let p = sync_trait_application(p.clone(), &trait_args, trait_ident.as_ref())?;
        preds.push(apply_mapping(p, m));
    }
    let items = item
        .items
        .iter()
        .map(|it| apply_mapping(it.to_token_stream(), m))
        .map(|it| expand_fresh_marks(it, fresh_names, template_segs, m))
        .collect::<Result<Vec<_>, _>>()?;
    // **The one renderer.** Both front-ends hand `render_impl` an `ImplParts`;
    // this entry's inputs stay token-level, so it fills the parts it has: the
    // generic params verbatim (`(param, None)` — their bounds ride inside the
    // param tokens, and the `<...>` joining rule lives in the renderer), the
    // whole body as one stream (associated-type items included), the item's own
    // attributes, the `unsafe` flag, and a target that stays the opaque
    // `TyPrimitive` catch-all: the renderer emits the caller's `target_tokens`,
    // so nothing reads it (a future consumer would parse it *then*, at the point
    // that has a reason to). The trait path is verbatim user Rust, so it carries
    // no `@N..` placeholders and travels as the whole path.
    let attrs = item.attrs.iter().map(|a| a.to_token_stream()).collect::<Vec<_>>();
    let body =
        if items.is_empty() { None } else { Some(items.into_iter().collect::<TokenStream>()) };
    let parts = ImplParts {
        impl_generics: all_params.into_iter().map(|p| (p, None)).collect(),
        trait_generic_names: vec![],
        associated_types: vec![],
        target_type: TyPrimitive(for_ty.clone()).to_ty(),
        body,
        attrs,
        is_unsafe_impl: item.unsafety.is_some(),
        where_clauses: vec![],
        impl_templates: vec![],
        shape_templates: vec![],
        fresh_binding: None,
        body_at: false,
    };
    let trait_tokens = trait_path.map(|p| trait_path_with_mapped_args(p, m));
    Ok(crate::codegen::render_impl(parts, preds, for_ty, trait_tokens.as_ref(), false, None))
}

/// Reconciles the attr's `new-generic-decl` with the params the impl block
/// already declares, and with the names the slot mapping replaced.
///
/// One authority for "which generics does the impl end up declaring": the item's
/// own params win (the for-Type / body / trait path reference them), a name that
/// is already declared — or that the mapping replaced — is **not** declared a
/// second time (rustc would report `E0403`), and the dropped declaration's bounds
/// move into a where predicate so nothing is silently lost
/// (`#[batch_impl(<T: Clone> Box<T>)] impl<T> Box<T>` keeps `T: Clone` as
/// `where T: Clone`).
///
/// Returns the declarations to keep and the predicates for the dropped ones. An
/// unparsable declaration is kept verbatim (the shape parser owns that input).
fn reconcile_new_gen(
    new_gen: &TokenStream, item_param_names: &std::collections::HashSet<String>,
    slot_names: &std::collections::HashSet<&str>,
) -> (Vec<TokenStream>, Vec<TokenStream>) {
    // The decl is one angle group's *contents* (`<T: Clone>` → `T: Clone`), so it
    // reads as a `Generics` body once re-wrapped.
    let Ok(generics) = syn::parse2::<syn::Generics>(quote!(< #new_gen >)) else {
        return (vec![new_gen.clone()], vec![]);
    };
    let mut kept = vec![];
    let mut dropped = vec![];
    for p in generics.params {
        let name = crate::ast::name_of_generic_param(&p);
        // A name matches a slot exactly: the slot set holds bare type/const
        // names, and a lifetime's spelling (`'a`) is never one of them — a
        // lifetime ident is not a slot position (`apply_mapping` leaves it
        // alone), so treating `'a` as the slot `a` would drop a declaration that
        // is still used (E0261).
        let already = item_param_names.contains(&name) || slot_names.contains(name.as_str());
        if already {
            if let syn::GenericParam::Type(tp) = &p
                && !tp.bounds.is_empty()
            {
                let (ident, bounds) = (&tp.ident, &tp.bounds);
                dropped.push(quote!(#ident: #bounds));
            }
            continue;
        }
        kept.push(p.to_token_stream());
    }
    (kept, dropped)
}

/// The trait path with the slot mapping applied to its **angle arguments**.
///
/// The path's own idents are left alone (a slot that happens to share the trait's
/// last segment name must not rename the trait reference), while an argument that
/// names a slot is substituted: `#[batch_impl(Wrapper<T> : [Box, Rc].u8)] impl<T>
/// PartialEq<T> for Wrapper<T>` strips the parameter, so leaving `T` in the trait
/// argument would reach the compiler unresolved (`E0425` — F2 of the review
/// pass; only the for-Type / where / body were mapped). Parenthesised arguments
/// (`Fn(..)` sugar) and a mapping with no slots are passed through untouched, so
/// every impl that never used a slot renders byte-identically.
fn trait_path_with_mapped_args(path: &syn::Path, m: &Mapping) -> TokenStream {
    if m.slots().is_empty() {
        return path.to_token_stream();
    }
    let mut out = TokenStream::new();
    if path.leading_colon.is_some() {
        out.extend(quote!(::));
    }
    for (i, seg) in path.segments.iter().enumerate() {
        if i > 0 {
            out.extend(quote!(::));
        }
        out.extend(seg.ident.to_token_stream());
        match &seg.arguments {
            syn::PathArguments::AngleBracketed(ab) => {
                let mapped = ab.args.iter().map(|a| apply_type_mapping(a.to_token_stream(), m));
                if ab.colon2_token.is_some() {
                    out.extend(quote!(::));
                }
                out.extend(quote!(< #(#mapped),* >));
            }
            syn::PathArguments::Parenthesized(p) => out.extend(p.to_token_stream()),
            syn::PathArguments::None => {}
        }
    }
    out
}

/// Expands `fresh!(...)` markers in the item body: the group's content is
/// DSL — repeat blocks (`@(...)..`), `@ident` = a segment reference (a
/// **template segment** from the shape form's `ident@..`, or an implicit
/// segment bound to this impl's fresh generics), `@{N}` = the N-th fresh
/// name. `fresh!` is an invisible internal marker (the attribute entry's
/// repeat protocol, wrapped in a macro-call spelling so the body stays
/// legal Rust): the call is fully expanded here and never reaches the
/// output — the user never defines a `fresh` macro.
fn expand_fresh_marks(
    tokens: TokenStream, fresh_names: &[TokenStream], template_segs: &[VarSeg], m: &Mapping,
) -> Result<TokenStream, TokenStream> {
    let v = tokens.into_iter().collect::<Vec<_>>();
    let mut out = vec![];
    let mut i = 0;
    while let Some(cur) = v.get(i) {
        // `fresh ! ( ... )` — the marker.
        if let TokenTree::Ident(id) = cur
            && id == "fresh"
            && matches!(v.get(i + 1), Some(TokenTree::Punct(p)) if p.as_char() == '!')
        {
            let Some(g) = crate::util::group_at(&v, i + 2, delimiter![()]) else {
                out.push(cur.clone());
                i += 1;
                continue;
            };
            let inner = g.stream().into_iter().collect::<Vec<_>>();
            out.extend(expand_fresh_inner(&inner, fresh_names, template_segs, m)?);
            i += 3;
            continue;
        }
        // Recurse into every other group (attributes, tuple literals, ...).
        if let TokenTree::Group(g) = cur {
            let inner = expand_fresh_marks(g.stream(), fresh_names, template_segs, m)?;
            let mut ng = Group::new(g.delimiter(), inner);
            ng.set_span(g.span());
            out.push(TokenTree::Group(ng));
            i += 1;
            continue;
        }
        out.push(cur.clone());
        i += 1;
    }
    Ok(out.into_iter().collect())
}

/// Expands one `fresh!(...)` group against this impl's fresh names: a
/// template segment (`(T@..)` from the shape form) drives the repeat blocks
/// with its matched leaf values; an `@ident` outside the template segments
/// declares an implicit segment bound to the fresh list. `@{N}` resolves to
/// the N-th fresh. Reuses the attribute entry's repeat machinery verbatim.
fn expand_fresh_inner(
    inner: &[TokenTree], fresh_names: &[TokenStream], template_segs: &[VarSeg], m: &Mapping,
) -> Result<Vec<TokenTree>, TokenStream> {
    let mut segs = template_segs.to_vec();
    let mut map = Mapping::default();
    // Template segments first (values come from the shape match's mapping).
    for s in template_segs {
        for k in 0..s.len {
            let pos = s.start + k;
            if let Some(v) = m.seg_value(&s.prefix, pos) {
                map.bind_seg(&s.prefix, pos, v.clone())
                    .map_err(|e| compile_error_str(&e.message(), proc_macro2::Span::call_site()))?;
            }
        }
    }
    collect_fresh_segment(inner, fresh_names, &mut segs, &mut map)?;
    let fresh = FreshCtx {
        names: fresh_names.iter().enumerate().map(|(i, n)| (0, i, n.clone())).collect(),
    };
    let cx = RepeatCtx {
        segs: &segs,
        map: &map,
        fresh: &fresh,
        binding: None,
        budget: Cell::new(MAX_REPEAT_TOKENS),
    };
    // Repeat blocks + segments first (the `@{...}` carriers inside a block
    // resolve in `substitute`); top-level carriers pass through and resolve
    // here.
    let expanded = expand_repeat_blocks(inner.iter().cloned().collect(), &cx)?;
    crate::codegen::expand_range_refs(expanded, &fresh).map(|o| o.into_iter().collect())
}

/// Collects the implicit segments of one `fresh!(...)` group: an `@ident`
/// reference (groups recursed) declares a segment whose elements are this
/// impl's fresh names (`T` → `T0 := P0, T1 := P1, ...`). A `fresh!` with no
/// freshs to bind is an error. `@{...}` carriers and `@N` cursors are not
/// segments.
fn collect_fresh_segment(
    tokens: &[TokenTree], fresh_names: &[TokenStream], segs: &mut Vec<VarSeg>, map: &mut Mapping,
) -> Result<(), TokenStream> {
    let mut i = 0;
    while let Some(cur) = tokens.get(i) {
        if is_punct_at(tokens, i, '@')
            && let Some(TokenTree::Ident(id)) = tokens.get(i + 1)
            && !matches!(tokens.get(i + 2), Some(TokenTree::Group(_)))
        {
            let prefix = id.to_string();
            if !segs.iter().any(|s| s.prefix == prefix) {
                if fresh_names.is_empty() {
                    return Err(compile_error_str(
                        &format!(
                            "batch-impl: `fresh!` references `@{}` but this impl has no \
                             fresh generics (no generator in the target)",
                            prefix,
                        ),
                        id.span(),
                    ));
                }
                segs.push(VarSeg { prefix: prefix.clone(), start: 0, len: fresh_names.len() });
                for (k, n) in fresh_names.iter().enumerate() {
                    map.bind_seg(&prefix, k, n.clone())
                        .map_err(|e| compile_error_str(&e.message(), id.span()))?;
                }
            }
        }
        if let TokenTree::Group(g) = cur {
            collect_fresh_segment(
                &g.stream().into_iter().collect::<Vec<_>>(),
                fresh_names,
                segs,
                map,
            )?;
        }
        i += 1;
    }
    Ok(())
}

/// Parses a matrix-source (DSL expression) into its leaf types.
pub(crate) fn parse_matrix_leaves(matrix: &[TokenTree]) -> Result<Vec<Ty>, TokenStream> {
    let mut cursor = Cursor::new(matrix);
    let (leaves, errors) = collect_spec_leaves(&mut cursor, Op::Comma, None);
    if !errors.is_empty() {
        return Err(errors.into_iter().collect());
    }
    Ok(leaves)
}

/// Extracts the `where{...}` attachments of the **template region** (the
/// part before the shape colon — the template must stay a standard syn type,
/// so attachments are stripped here at the token level). The matrix region
/// keeps its `where{...}` attachments: the parse layer turns them into
/// `TyWithWhere` and the leaf extraction borrows the attribute entry's
/// predicate splitting ([`split_at_depth0`]). Multiple attachments are
/// comma-joined.
pub(crate) fn peel_where(spec: &[TokenTree]) -> (Vec<TokenTree>, Vec<TokenTree>) {
    // The template region ends at the depth-0 shape colon (or the stream end
    // for the direct form).
    let colon = find_shape_colon(spec).unwrap_or(spec.len());
    let mut out = vec![];
    let mut preds = vec![];
    let mut i = 0;
    while i < colon {
        let Some(cur) = spec.get(i) else { break };
        if let TokenTree::Ident(id) = cur
            && *id == "where"
        {
            let Some(g) = crate::util::group_at(spec, i + 1, delimiter![{}]) else {
                out.push(cur.clone());
                i += 1;
                continue;
            };
            if !preds.is_empty() {
                preds.push(TokenTree::Punct(proc_macro2::Punct::new(
                    ',',
                    proc_macro2::Spacing::Alone,
                )));
            }
            preds.extend(g.stream().into_iter().collect::<Vec<_>>());
            i += 2;
            continue;
        }
        out.push(cur.clone());
        i += 1;
    }
    out.extend(slice_from(spec, colon).iter().cloned());
    (out, preds)
}

/// The depth-0 single `:` that separates the shape template from the rest.
pub(crate) fn find_shape_colon(spec: &[TokenTree]) -> Option<usize> {
    spec.iter().enumerate().find_map(|(i, tt)| {
        matches!(tt, TokenTree::Punct(_) if is_single_colon(spec, i)).then_some(i)
    })
}

/// `new-generic-decl?` at the head: a `delimiter![<>]` group. Returns (decl
/// contents, rest).
///
/// A leading angle group that is a **qualified-self head** (`<T as Tr>` — a
/// depth-0 `as`, the same discriminator the parse layer uses) is **not** a
/// declaration: `<T as Tr>::Assoc` is a projection type, and swallowing its head
/// as the new-generic decl would leave a dangling `::Assoc` for the matrix
/// parser ("the direct form takes exactly one type").
pub(crate) fn split_new_gen(tokens: &[TokenTree]) -> (Option<TokenStream>, Vec<TokenTree>) {
    match tokens.first() {
        Some(TokenTree::Group(g))
            if g.delimiter() == delimiter![<>]
                && crate::parse::split_projection(&g.stream().into_iter().collect::<Vec<_>>())
                    .is_none() =>
        {
            (Some(g.stream()), slice_from(tokens, 1).to_vec())
        }
        _ => (None, tokens.to_vec()),
    }
}
