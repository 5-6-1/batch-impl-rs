//! Compile-time diagnostic construction.
//!
//! Central entry point; the error span is threaded through every call:
//! [`compile_err!`] keeps the macro call site (no span), while
//! [`compile_err_at!`] attaches an explicit span (``compile_err_at!(span,
//! "msg {}")``) — precise spans are added incrementally at the error points
//! where the offending token is in hand.

use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;

/// Build `::core::compile_error!(msg);` at `span` (the token the error is
/// about). The **absolute path** keeps the diagnostic hygienic: a user scope
/// shadowing `compile_error` (or defining its own `core` module) cannot
/// redirect the macro; the `compile_error` ident keeps the target span so
/// rustc reports the error at the offending token while the path prefix
/// keeps call-site spans (avoiding rustc treating it as user code in item
/// position — "macros that expand to items must be delimited with braces or
/// followed by a semicolon").
pub(crate) fn compile_error_str(msg: &str, span: Span) -> TokenStream {
    let err_ident = Ident::new("compile_error", span);
    quote! { :: core :: #err_ident!(#msg); }
}

/// Type-position `::core::compile_error!(msg)` without a trailing `;` —
/// inside generic args / type positions a semicolon is a syntax error; same
/// ident-span scheme as [`compile_error_str`].
pub(crate) fn compile_error_ty(msg: &str, span: Span) -> TokenStream {
    let err_ident = Ident::new("compile_error", span);
    quote! { :: core :: #err_ident!(#msg) }
}

/// `compile_err!("msg {}", x)` → `compile_error_str(&format!(...), call_site)`.
/// Self-contained via `$crate` / `::proc_macro2` (definition-site paths), so
/// use sites need no `compile_error_str` import.
macro_rules! compile_err {
    ($($t:tt)*) => {
        $crate::util::compile_error_str(
            &format!($($t)*),
            ::proc_macro2::Span::call_site(),
        )
    };
}
pub(crate) use compile_err;

/// `compile_err_at!(span, "msg {}", x)` → `compile_error_str(&format!(...), span)`.
/// Use where the offending token's span is in hand (parse cursor position,
/// `@` reference, directive argument group, `Ty::span`).
macro_rules! compile_err_at {
    ($span:expr, $($t:tt)*) => {
        $crate::util::compile_error_str(&format!($($t)*), $span)
    };
}
pub(crate) use compile_err_at;
