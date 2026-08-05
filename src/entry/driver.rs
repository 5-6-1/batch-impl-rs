use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::ItemTrait;

use crate::TraitBounds;
use crate::apply::err_ty;
use crate::ast::{Expand, Op};
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
pub(crate) fn parse_batch_trait_entry(
    cursor: &mut Cursor, top_level: Op, trait_full_path: &TokenStream,
    trait_last_ident: &Ident, is_unsafe_trait: bool, start_trait: Option<ItemTrait>,
    trait_bounds: &TraitBounds,
) -> TokenStream {
    let mut tys = vec![];
    // Leading comma (`#[batch_impl(,usize)]` / `A: ,usize`): the whole list starts with `,`.
    // With a streaming cursor, parse_item cannot tell a "leading comma" from a "separator
    // comma after the previous spec", so this check lives in this entry where the call
    // order is known.
    if cursor.is_punct(',') {
        tys.push(err_ty("batch-impl: spec list cannot start with `,`"));
    }
    while let Some(ty) = parse_item(cursor, top_level, trait_last_ident.into()) {
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
    let mut impls = start_trait.map_or(quote![], |t| quote![#t]);
    for t in tys {
        impls.extend(generate_impl(
            t,
            trait_full_path,
            is_unsafe_trait,
            trait_bounds,
        ));
    }
    impls
}
