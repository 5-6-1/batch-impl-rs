//! Compile-time diagnostic construction.
//!
//! Central entry point; a future diagnostic structure carrying Span only needs changes here.

use proc_macro2::TokenStream;
use quote::quote;

/// Build `compile_error!(msg);` for compile-time errors.
pub(crate) fn compile_error_str(msg: &str) -> TokenStream {
    quote! { compile_error!(#msg); }
}

/// `compile_err!("msg {}", x)` expands to `compile_error_str(&format!(...))`.
macro_rules! compile_err {
    ($($t:tt)*) => {
        compile_error_str(&format!($($t)*))
    };
}
pub(crate) use compile_err;
