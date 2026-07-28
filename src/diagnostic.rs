//! 编译期诊断构造。
//!
//! v0.4.2 抽出的集中入口。原本散落在 `lib.rs` 与 `preprocess.rs` 的
//! 两份同名 `compile_error` 函数全部汇聚到 [`compile_error_str`]，
//! 防止诊断构造点散乱漂移。未来若要引入带 `Span` 的诊断结构
//! （更精细的 IDE 高亮），只需改本模块一处。

use proc_macro2::TokenStream;
use quote::quote;

/// 构造 `compile_error!(msg);` 用于编译期报错。
///
/// 统一了原本散落在 `lib.rs` 与 `preprocess.rs` 中的同名函数，
/// 任何编译期诊断应通过本函数发出（保持"永不 panic"原则）。
pub(crate) fn compile_error_str(msg: &str) -> TokenStream {
    quote! { compile_error!(#msg); }
}
