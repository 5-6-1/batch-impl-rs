use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::ItemTrait;

use crate::TraitBounds;
use crate::apply::err_ty;
use crate::ast::{Expand, Op, Ty, TyKind, reset_fresh_counter};
use crate::codegen::generate_impl;
use crate::parse::parse_item;
use crate::util::Cursor;

/// Shared driver: parse impl-specs from the cursor, expand parallel lists, and generate
/// impl blocks.
///
/// `top_level` controls the top-level precedence:
/// - `Op::Comma` for `#[batch_impl]` (the whole argument is separated by `,`)
/// - `Op::Comma` for a single `batch_trait!` segment's specs too (the `;`
///   segment boundary is pre-cut by `take_segment`; `Op::Semi` is used only
///   inside array `[T; N]` parsing)
///
/// `trait_bounds`: inline bound mapping of the trait's generic params (param name → bound
/// tokens), letting `generate_impl` inherit bounds by position + name for impl generic params
/// without written bounds; `batch_trait!` passes an empty mapping since it has no trait
/// definition.
///
/// The expansion stage uses a work queue (a stack, reversed to preserve output order) to
/// flatten the parallel list `Ty::Array` into leaf `Ty`s, then calls `generate_impl` per
/// leaf to emit the corresponding impl block. Note: a bare code block `WithCode(None, ...)`
/// is also a leaf, injected verbatim as a top-level item by `generate_impl` (the carrier of
/// open instruction extensions).
// Pipeline entry with many context params (spec tokens, trait path/name,
// bounds, fresh-name list) — clippy's default 7-arg threshold is not useful
// here; a context struct would obscure the one-shot pipeline flow.
#[allow(clippy::too_many_arguments)]
pub(crate) fn parse_batch_trait_entry(
    cursor: &mut Cursor, top_level: Op, trait_full_path: &TokenStream,
    trait_last_ident: &Ident, is_unsafe_trait: bool, start_trait: Option<ItemTrait>,
    trait_bounds: &TraitBounds, trait_param_names: &[Ident],
) -> TokenStream {
    let (tys, errors) = collect_spec_leaves(cursor, top_level, trait_last_ident);
    if !errors.is_empty() {
        return errors.into_iter().collect();
    }
    let mut impls = start_trait.map_or(quote![], |t| quote![#t]);
    for t in tys {
        impls.extend(generate_impl(
            t,
            trait_full_path,
            is_unsafe_trait,
            trait_bounds,
            trait_param_names,
        ));
    }
    impls
}

/// Parses the cursor into leaf `Ty`s (specs → worklist expansion → leaves)
/// and aggregates every error. Shared by the three entries (via
/// [`parse_batch_trait_entry`]) and the preview entry (`batch_preview!`
/// inspects the leaves before generating) — the single authority for the
/// parse/expand stage, so the two consumers cannot drift apart.
///
/// Error aggregation: collect every spec's error (recursing into nested
/// wrappers — e.g. `Box<@0..=2>` carries the range error inside its
/// type params) and report them all at once; the old behavior stopped at
/// the first error, hiding later ones. When any error exists, the caller
/// emits only the errors — no partial impls.
pub(crate) fn collect_spec_leaves(
    cursor: &mut Cursor, top_level: Op, trait_last_ident: &Ident,
) -> (Vec<Ty>, Vec<TokenStream>) {
    let mut tys = vec![];
    // Leading comma (`#[batch_impl(,usize)]` / `A: ,usize`): the whole list starts with `,`.
    // With a streaming cursor, parse_item cannot tell a "leading comma" from a "separator
    // comma after the previous spec", so this check lives in this entry where the call
    // order is known.
    if cursor.is_punct(',') {
        tys.push(err_ty("batch-impl: spec list cannot start with `,`"));
    }
    while let Some(ty) = parse_item(cursor, top_level, trait_last_ident.into()) {
        // Fresh-generator group ids are DSL-local: reset per spec so `@g_i`
        // (future) and the codegen sweep never depend on spec position.
        reset_fresh_counter();
        let mut queue = vec![ty];
        while let Some(item) = queue.pop() {
            match item.expand() {
                Expand::Many(expanded) => {
                    for e in expanded.into_iter().rev() {
                        queue.push(e);
                    }
                }
                Expand::Leaf(leaf) => tys.push(leaf),
            }
        }
    }
    let mut errors = vec![];
    for t in &tys {
        collect_errors(t, &mut errors);
    }
    (tys, errors)
}

fn collect_errors(ty: &Ty, out: &mut Vec<TokenStream>) {
    if let Ty { kind: TyKind::Error(e), .. } = ty {
        out.push(e.0.clone());
    }
    // Reuse map_children's exhaustive child list for the recursion
    // (rebuild-style pass; children visited in the same order).
    ty.clone().map_children(&mut |child| {
        collect_errors(&child, out);
        child
    });
}
