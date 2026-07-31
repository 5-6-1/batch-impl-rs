use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::ItemTrait;

use crate::codegen::generate_impl;
use crate::parse::parse_item;
use crate::scan::Cursor;
use crate::types::Op;

/// 共享驱动：从游标解析 impl-specs，展开并列列表，生成 impl 块。
///
/// `top_level` 控制顶层优先级：
/// - `Op::Comma` 用于 `#[batch_impl]`（整个参数按 `,` 分隔）
/// - `Op::Semi` 用于 `batch_trait!` 的单段 specs（按 `,` 分隔，遇到 `;` 段落边界停止）
///
/// 展开阶段通过 BFS 工作清单把 `Ty::Array`（并列列表）逐层摊平为叶子 `Ty`，
/// 再对每个叶子调用 `generate_impl` 生成对应的 impl 块。
pub(crate) fn parse_batch_trait_entry(
    cursor: &mut Cursor, top_level: Op, trait_full_path: &TokenStream,
    trait_last_ident: &Ident, is_unsafe_trait: bool, start_trait: Option<ItemTrait>,
) -> TokenStream {
    let mut tys = vec![];
    while let Some(ty) = parse_item(cursor, top_level, Some(trait_last_ident)) {
        let mut queue = vec![ty];
        while let Some(item) = queue.pop() {
            match item.expand() {
                Ok(expanded) => {
                    for e in expanded.into_iter().rev() {
                        queue.push(e);
                    }
                }
                Err(leaf) => tys.push(leaf),
            }
        }
    }
    let mut impls = start_trait.map_or(quote![], |t| quote![#t]);
    for t in tys {
        impls.extend(generate_impl(t, trait_full_path, is_unsafe_trait));
    }
    impls
}
