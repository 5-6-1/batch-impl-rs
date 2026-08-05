//! trait 定义语义分析：泛型 bound 收集、where 谓词引用收集。
//!
//! 从 `syn::ItemTrait` 提取结构化信息（[`TraitBounds`]），供 codegen 的
//! bound 继承、preprocess 的 `A<>` 照抄与 `#blanket` 泛型实参复用。
//! `mod.rs` 聚合 re-export，引用侧写 `crate::analyze::X`。

pub(crate) mod trait_bounds;

pub(crate) use trait_bounds::*;
