use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{quote, quote_spanned};
use crate::core::types::ImplSpec;

// ===========================================================================
// 代码生成
// ===========================================================================

pub fn generate_impl(spec: &ImplSpec, trait_name_tokens: TokenStream2) -> TokenStream2 {
    let span = spec
        .target
        .clone()
        .into_iter()
        .next()
        .map(|t| t.span())
        .unwrap_or_else(Span::call_site);
    let target = &spec.target;
    let body = spec.custom_body.clone().unwrap_or_default();

    let impl_generics = if spec.type_params.is_empty() {
        quote! {}
    } else {
        let p = &spec.type_params;
        quote! { < #(#p),* > }
    };

    let trait_path = match &spec.trait_params {
        Some(params) if !params.is_empty() => quote! { #trait_name_tokens < #(#params),* > },
        _ => quote! { #trait_name_tokens },
    };

    let assoc = if spec.assoc_bindings.is_empty() {
        quote! {}
    } else {
        let bindings: Vec<_> = spec.assoc_bindings.iter().map(|(name, value)| {
            quote! { type #name = #value; }
        }).collect();
        quote! { #(#bindings)* }
    };

    let attrs = if spec.attributes.is_empty() {
        quote! {}
    } else {
        let attr_list = &spec.attributes;
        quote! { #(#attr_list)* }
    };

    if spec.is_unsafe {
        quote_spanned! { span =>
            #attrs
            unsafe impl #impl_generics #trait_path for #target { #assoc #body }
        }
    } else {
        quote_spanned! { span =>
            #attrs
            impl #impl_generics #trait_path for #target { #assoc #body }
        }
    }
}
