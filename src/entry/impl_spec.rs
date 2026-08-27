//! Impl assembly and spec helpers for the impl entry (kept under the
//! 350-line cap by living in their own file): `assemble_impl` renders one
//! generated impl from the extracted parts; the small helpers parse the
//! matrix source and split the shape-form spec.

use proc_macro2::{TokenStream, TokenTree};
use quote::{ToTokens, quote};
use syn::ItemImpl;

use crate::ast::{Op, Ty};
use crate::codegen::{Mapping, apply_mapping, sync_trait_application};
use crate::entry::driver::collect_spec_leaves;
use crate::util::{Cursor, is_single_colon};

/// Assembles one generated impl: generics (attr new-generic-decl first, then
/// the hoisted fresh names, then the impl's own params), trait path
/// (**`None` for an inherent impl** — the `for` section is omitted and the
/// rewritten self type stands alone), merged where clause, rewritten body.
/// `m` is the slot mapping (empty for the direct form / empty matrix).
#[allow(clippy::too_many_arguments)]
pub(crate) fn assemble_impl(
    item: &ItemImpl, trait_path: Option<&syn::Path>, new_gen: Option<&TokenStream>,
    fresh_names: &[TokenStream], where_preds: &[TokenStream], m: &Mapping, for_ty: TokenStream,
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
    let items =
        item.items.iter().map(|it| apply_mapping(it.to_token_stream(), m)).collect::<Vec<_>>();
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

/// Parses a matrix-source (DSL expression) into its leaf types.
pub(crate) fn parse_matrix_leaves(matrix: &[TokenTree]) -> Result<Vec<Ty>, TokenStream> {
    let mut cursor = Cursor::new(matrix);
    let (leaves, errors) = collect_spec_leaves(&mut cursor, Op::Comma, None);
    if !errors.is_empty() {
        return Err(errors.into_iter().collect());
    }
    Ok(leaves)
}

/// `where{...}` tail (the where_process output shape) → (spec without the
/// where, predicate tokens).
pub(crate) fn peel_where(spec: &[TokenTree]) -> (&[TokenTree], Vec<TokenTree>) {
    if spec.len() >= 2
        && let Some(TokenTree::Group(g)) = spec.last()
        && g.delimiter() == delimiter![{}]
        && let Some(TokenTree::Ident(w)) = spec.get(spec.len() - 2)
        && *w == "where"
    {
        (&spec[..spec.len() - 2], g.stream().into_iter().collect())
    } else {
        (spec, vec![])
    }
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
