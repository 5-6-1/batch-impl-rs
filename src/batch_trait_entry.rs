use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::ItemTrait;

use crate::apply::err_ty;
use crate::codegen::generate_impl;
use crate::parse::parse_item;
use crate::scan::Cursor;
use crate::types::{Expand, Op};

/// 共享驱动：从游标解析 impl-specs，展开并列列表，生成 impl 块。
///
/// `top_level` 控制顶层优先级：
/// - `Op::Comma` 用于 `#[batch_impl]`（整个参数按 `,` 分隔）
/// - `Op::Semi` 用于 `batch_trait!` 的单段 specs（按 `,` 分隔，遇到 `;` 段落边界停止）
///
/// 展开阶段用工作清单（栈，倒序入栈以保持输出顺序）把并列列表 `Ty::Array`
/// 逐层摊平为叶子 `Ty`，再对每个叶子调用 `generate_impl` 生成对应的 impl 块。
/// 注意：裸代码块 `WithCode(None, ...)` 也是叶子，经 `generate_impl` 原样作为
/// 顶层 item 注入输出（开放指令扩展的载体）。
pub(crate) fn parse_batch_trait_entry(
    cursor: &mut Cursor, top_level: Op, trait_full_path: &TokenStream,
    trait_last_ident: &Ident, is_unsafe_trait: bool, start_trait: Option<ItemTrait>,
) -> TokenStream {
    let mut tys = vec![];
    // 前导逗号（`#[batch_impl(,usize)]` / `A: ,usize`）：整段列表以 `,` 开头。
    // 流式游标下 parse_item 无法区分"前导逗号"与"上一个 spec 后的分隔逗号"，
    // 只能在知道调用序的此入口判定。
    if cursor.is_punct(',') {
        tys.push(err_ty("batch-impl: spec 列表不能以 `,` 开头"));
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
        impls.extend(generate_impl(t, trait_full_path, is_unsafe_trait));
    }
    impls
}
