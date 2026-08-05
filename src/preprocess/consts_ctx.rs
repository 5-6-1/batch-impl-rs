//! `@` 常量来源上下文（`expand_consts` 一趟解析的三种来源并集）。
//!
//! 内置常量（`@uint`/`@scalar`/范围族）两种上下文都可用；
//! trait 感知常量（`@trait`/`@all` 系/`@Cow`）仅属性宏入口有。

use std::collections::HashMap;

use proc_macro2::{TokenStream, TokenTree};

/// `@` 常量的来源上下文：
/// - [`ConstCtx::Attribute`]：`#[batch_impl]`/`#[batch_impl_only]`——内置 + trait 感知
///   （`@trait`/`@all` 系/`@Cow`；`trait_full_path` 供 `@trait` 展开为完整路径，
///   `batch_impl_only` 外部 trait 场景即 `#ext::Trait:` 前缀路径）；
/// - [`ConstCtx::Trait`]：`batch_trait!`——内置 + 自定义表（前导 `@name=值;`）。
#[derive(Clone, Copy)]
pub(crate) enum ConstCtx<'a> {
    Attribute {
        trait_def: &'a syn::ItemTrait,
        trait_full_path: &'a TokenStream,
    },
    Trait {
        user_table: &'a UserConsts,
    },
}

pub(crate) type UserConsts = HashMap<String, Vec<TokenTree>>;

impl<'a> ConstCtx<'a> {
    /// 自定义常量表（仅 `batch_trait!` 有；属性宏不支持自定义定义）。
    pub(crate) fn user_table(&self) -> Option<&'a UserConsts> {
        match self {
            ConstCtx::Trait { user_table } => Some(user_table),
            ConstCtx::Attribute { .. } => None,
        }
    }

    /// trait 定义（仅属性宏入口有；`batch_trait!` 是函数式宏、拿不到定义）。
    pub(crate) fn trait_def(&self) -> Option<&'a syn::ItemTrait> {
        match self {
            ConstCtx::Attribute { trait_def, .. } => Some(trait_def),
            ConstCtx::Trait { .. } => None,
        }
    }

    /// trait 完整路径（`batch_impl` = 本地名；`batch_impl_only` = 外部路径）。
    pub(crate) fn trait_full_path(&self) -> Option<&'a TokenStream> {
        match self {
            ConstCtx::Attribute { trait_full_path, .. } => Some(trait_full_path),
            ConstCtx::Trait { .. } => None,
        }
    }
}
