//! 编译期诊断构造。
//!
//! 集中入口；未来带 Span 的诊断结构只需改本模块。

use proc_macro2::TokenStream;
use quote::quote;

/// 构造 `compile_error!(msg);` 用于编译期报错。
pub(crate) fn compile_error_str(msg: &str) -> TokenStream {
    quote! { compile_error!(#msg); }
}

/// `compile_err!("msg {}", x)` 展开为 `compile_error_str(&format!(...))`。
macro_rules! compile_err {
    ($($t:tt)*) => {
        compile_error_str(&format!($($t)*))
    };
}
pub(crate) use compile_err;
