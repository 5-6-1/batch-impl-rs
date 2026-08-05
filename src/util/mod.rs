//! 共享工具：token 游标扫描（[`Cursor`]）与编译期诊断构造（[`compile_error_str`]）。
//!
//! 本目录无业务依赖，被所有层引用；`mod.rs` 聚合 re-export，
//! 引用侧统一写 `crate::util::X`（不写子模块路径）。

pub(crate) mod diagnostic;
pub(crate) mod scan;

pub(crate) use diagnostic::*;
pub(crate) use scan::*;
