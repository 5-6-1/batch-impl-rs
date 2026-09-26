//! Shared fresh-name and constraint handling for both hand-written impl forms.

use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

use crate::ast::Ty;
use crate::ast::fresh_protocol::decl_fresh_pos;
use crate::codegen::{FreshCtx, collect_used_surfaces, resolve_where_predicates};

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

/// `assemble_impl` receives fresh names separately from predicates. Preserve
/// their hoisted bounds by sending them through the same reference resolver
/// as written where clauses; later shape/trait synchronization stays shared.
pub(super) fn resolve_fresh_predicates(
    declarations: &[(TokenStream, Option<Ty>)], clauses: &[TokenStream], ctx: &FreshCtx,
) -> Result<Vec<TokenStream>, TokenStream> {
    let mut clauses = clauses.to_vec();
    for (name, bound) in declarations {
        if decl_fresh_pos(name).is_some()
            && let Some(bound) = bound
        {
            clauses.push(quote!(#name: #bound));
        }
    }
    resolve_where_predicates(&clauses, ctx).map_err(|errors| errors.into_iter().collect())
}
