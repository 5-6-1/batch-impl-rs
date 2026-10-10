//! Hoisted declarations, fresh names and constraints for explicit impl specs.

use std::collections::HashSet;

use proc_macro2::{TokenStream, TokenTree};
use quote::{ToTokens, quote};

use crate::ast::fresh_protocol::decl_fresh_pos;
use crate::ast::{ParamKind, Ty, decl_fresh_group};
use crate::codegen::{
    FreshCtx, collect_used_surfaces, expand_range_refs, resolve_where_predicates,
};
use crate::preprocess::render_angles;

/// Hoisting removes declaration bounds from the target type. They remain user
/// text, just like the leaf's attachments, and must participate in naming.
pub(super) fn fresh_context(
    declarations: &[(TokenStream, Option<Ty>)], used: &HashSet<String>, surfaces: &[&TokenStream],
) -> FreshCtx {
    let mut used = used.clone();
    collect_used_surfaces(surfaces, &mut used);
    for (name, bound) in declarations {
        collect_used_surfaces(&[name], &mut used);
        if let Some(bound) = bound {
            collect_used_surfaces(&[&bound.to_token_stream()], &mut used);
        }
    }
    let names = declarations.iter().map(|(name, _)| name.clone()).collect::<Vec<_>>();
    FreshCtx::new(&names, &used)
}

/// Preserve bounds from every hoisted type/lifetime declaration. A const's
/// annotation is its type, not a predicate; it stays on its declaration below.
pub(super) fn resolve_hoisted_predicates(
    declarations: &[(TokenStream, Option<Ty>)], clauses: &[TokenStream], ctx: &FreshCtx,
) -> Result<Vec<TokenStream>, TokenStream> {
    let mut clauses = clauses.to_vec();
    for (name, bound) in declarations {
        if !ParamKind::of_name(name).is_const()
            && let Some(bound) = bound
        {
            clauses.push(quote!(#name: #bound));
        }
    }
    resolve_where_predicates(&clauses, ctx).map_err(|errors| errors.into_iter().collect())
}

/// Join user declarations with the written outer declaration; assembly owns
/// name reconciliation. Fresh names and empty-group metadata stay in FreshCtx.
pub(super) fn merge_hoisted_declarations(
    written: Option<&TokenStream>, declarations: &[(TokenStream, Option<Ty>)], ctx: &FreshCtx,
) -> Result<Option<TokenStream>, TokenStream> {
    let mut combined = written.cloned().map(render_angles).unwrap_or_default();
    for (name, bound) in declarations {
        if decl_fresh_pos(name).is_some() || decl_fresh_group(name).is_some() {
            continue;
        }
        let declaration = if ParamKind::of_name(name).is_const()
            && let Some(ty) = bound
        {
            quote!(#name: #ty)
        } else {
            name.clone()
        };
        let declaration = render_angles(expand_range_refs(declaration, ctx)?);
        if !combined.is_empty()
            && !matches!(combined.clone().into_iter().last(), Some(TokenTree::Punct(p)) if p.as_char() == ',')
        {
            combined.extend(quote!(,));
        }
        combined.extend(declaration);
    }
    Ok((!combined.is_empty()).then_some(combined))
}
