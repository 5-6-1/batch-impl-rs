//! Impl assembly and spec helpers for the impl entry (kept under the
//! 350-line cap by living in their own file): `assemble_impl` renders one
//! generated impl from the extracted parts; the small helpers parse the
//! matrix source and split the shape-form spec.

use proc_macro2::{Group, TokenStream, TokenTree};
use quote::{ToTokens, quote};
use std::cell::Cell;
use syn::ItemImpl;

use crate::ast::{Op, Ty};
use crate::codegen::FreshCtx;
use crate::codegen::{
    MAX_REPEAT_TOKENS, Mapping, RepeatCtx, VarSeg, apply_mapping, expand_repeat_blocks,
    sync_trait_application,
};
use crate::entry::driver::collect_spec_leaves;
use crate::util::{Cursor, compile_error_str, is_punct_at, is_single_colon};

/// Assembles one generated impl: generics (attr new-generic-decl first, then
/// the hoisted fresh names, then the impl's own params), trait path
/// (**`None` for an inherent impl** — the `for` section is omitted and the
/// rewritten self type stands alone), merged where clause, rewritten body.
/// `m` is the slot mapping (empty for the direct form / empty matrix).
#[allow(clippy::too_many_arguments)]
pub(crate) fn assemble_impl(
    item: &ItemImpl, trait_path: Option<&syn::Path>, new_gen: Option<&TokenStream>,
    fresh_names: &[TokenStream], where_preds: &[TokenStream], m: &Mapping,
    template_segs: &[VarSeg], for_ty: TokenStream,
) -> Result<TokenStream, TokenStream> {
    let item_params = item.generics.params.iter().map(|p| p.to_token_stream()).collect::<Vec<_>>();
    // Generics: the attr new-generic-decl first, then the hoisted fresh names
    // (`P0, P1, ...` from a generator in the target), then the impl's own
    // params.
    let mut all_params = Vec::with_capacity(
        new_gen.map_or(0, |n| n.clone().into_iter().count())
            + fresh_names.len()
            + item_params.len(),
    );
    if let Some(ng) = new_gen
        && !ng.clone().into_iter().next().is_none()
    {
        all_params.push(ng.clone());
    }
    all_params.extend(fresh_names.iter().cloned());
    all_params.extend(item_params);
    let gen_tokens = if all_params.is_empty() { quote!() } else { quote!(<#(#all_params),*>) };
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
    let mut preds = vec![];
    for p in where_preds {
        let p = sync_trait_application(p.clone(), &trait_args)?;
        preds.push(apply_mapping(p, m));
    }
    if let Some(wc) = &item.generics.where_clause {
        let p = sync_trait_application(wc.predicates.to_token_stream(), &trait_args)?;
        preds.push(apply_mapping(p, m));
    }
    let where_clause = if preds.is_empty() { quote!() } else { quote!(where #(#preds),*) };
    let items = item
        .items
        .iter()
        .map(|it| apply_mapping(it.to_token_stream(), m))
        .map(|it| expand_fresh_marks(it, fresh_names, template_segs, m))
        .collect::<Result<Vec<_>, _>>()?;
    let unsafe_kw = if item.unsafety.is_some() { quote!(unsafe) } else { quote!() };
    let head = match trait_path {
        Some(p) => quote!(impl #gen_tokens #p for #for_ty),
        None => quote!(impl #gen_tokens #for_ty),
    };
    Ok(quote! {
        #unsafe_kw #head #where_clause {
            #(#items)*
        }
    })
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
    while i < v.len() {
        // `fresh ! ( ... )` — the marker.
        if let TokenTree::Ident(id) = &v[i]
            && id == "fresh"
            && matches!(v.get(i + 1), Some(TokenTree::Punct(p)) if p.as_char() == '!')
            && matches!(v.get(i + 2), Some(TokenTree::Group(g))
                if g.delimiter() == delimiter![()])
        {
            let TokenTree::Group(g) = &v[i + 2] else { unreachable!("matched above") };
            let inner = g.stream().into_iter().collect::<Vec<_>>();
            out.extend(expand_fresh_inner(&inner, fresh_names, template_segs, m)?);
            i += 3;
            continue;
        }
        // Recurse into every other group (attributes, tuple literals, ...).
        if let TokenTree::Group(g) = &v[i] {
            let inner = expand_fresh_marks(g.stream(), fresh_names, template_segs, m)?;
            let mut ng = Group::new(g.delimiter(), inner);
            ng.set_span(g.span());
            out.push(TokenTree::Group(ng));
            i += 1;
            continue;
        }
        out.push(v[i].clone());
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
    while i < tokens.len() {
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
        if let TokenTree::Group(g) = &tokens[i] {
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
        if let TokenTree::Ident(id) = &spec[i]
            && *id == "where"
            && matches!(spec.get(i + 1), Some(TokenTree::Group(g))
                if g.delimiter() == delimiter![{}])
        {
            let TokenTree::Group(g) = &spec[i + 1] else { unreachable!("matched above") };
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
        out.push(spec[i].clone());
        i += 1;
    }
    out.extend(spec[colon..].iter().cloned());
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
pub(crate) fn split_new_gen(tokens: &[TokenTree]) -> (Option<TokenStream>, Vec<TokenTree>) {
    match tokens.first() {
        Some(TokenTree::Group(g)) if g.delimiter() == delimiter![<>] => {
            (Some(g.stream()), tokens[1..].to_vec())
        }
        _ => (None, tokens.to_vec()),
    }
}
