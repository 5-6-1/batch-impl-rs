use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use crate::types::*;

/// 从 Ty 中递归提取 impl 块所需的各部分。
///
/// `impl_spec` 的 AST 节点是按修饰顺序嵌套的（如 `<T> Trait<T> unsafe Box<T> { body }`），
/// 此函数沿树递归拆解，收集：impl 泛型、trait 泛型、关联类型绑定、目标类型、body、属性、unsafe 标记。
pub(crate) struct ImplParts {
    pub(crate) impl_generics: Vec<(TokenStream, Option<Ty>)>,
    pub(crate) trait_generic_names: Vec<TokenStream>,
    pub(crate) associated_types: Vec<(TokenStream, TokenStream)>,
    pub(crate) target_type: Ty,
    pub(crate) body: Option<TokenStream>,
    pub(crate) attrs: Vec<TokenStream>,
    pub(crate) is_unsafe_impl: bool,
}

impl ImplParts {
    /// 叶子节点：无任何修饰，仅目标类型
    fn leaf(target_type: Ty) -> Self {
        ImplParts {
            impl_generics: vec![],
            trait_generic_names: vec![],
            associated_types: vec![],
            target_type,
            body: None,
            attrs: vec![],
            is_unsafe_impl: false,
        }
    }
}

/// 递归拆解 Ty 树，提取 impl 块所需的全部元数据。
///
/// 每遇到一个包装节点就剥离其贡献（泛型、绑定、属性、unsafe），递归处理内层，
/// 直到遇到叶子节点（纯目标类型）。
pub(crate) fn extract_impl_parts(ty: Ty) -> ImplParts {
    match ty {
        Ty::WithType(wt) => {
            let mut parts = extract_impl_parts(*wt.1);
            parts.impl_generics.extend(wt.0.params);
            parts.associated_types.extend(wt.0.bindings);
            parts
        }
        Ty::WithTrait(wt) => {
            let mut parts = extract_impl_parts(*wt.1);
            parts.trait_generic_names.extend(wt.0.1.params.into_iter().map(|p| p.0));
            parts.associated_types.extend(wt.0.1.bindings);
            parts
        }
        Ty::WithCode(wc) => {
            let mut parts = extract_impl_parts(*wc.0);
            match &mut parts.body {
                Some(t) => t.extend(wc.1),
                None => parts.body = Some(wc.1),
            }
            parts
        }
        Ty::WithAttr(wa) => {
            let mut parts = extract_impl_parts(*wa.1);
            let stream = &wa.0.0;
            parts.attrs.push(quote!(#[#stream]));
            parts
        }
        Ty::Unsafe(u) => {
            let mut parts = extract_impl_parts(*u.0);
            parts.is_unsafe_impl = true;
            parts
        }
        Ty::Modified(m) => {
            let mut parts = extract_impl_parts(*m.1);
            parts.target_type = TyModified(m.0, Box::new(parts.target_type)).into();
            parts
        }
        other => ImplParts::leaf(other),
    }
}

/// 生成一个 impl 块：拆解元数据 → 构建泛型参数 / trait 泛型 / impl body → 输出 `quote!` 块
pub(crate) fn generate_impl(ty: Ty, trait_name: &TokenStream, is_unsafe_trait: bool) -> TokenStream {
    let parts = extract_impl_parts(ty);

    let is_unsafe = is_unsafe_trait || parts.is_unsafe_impl;
    let unsafe_kw = if is_unsafe { quote!(unsafe) } else { quote!() };

    // impl 泛型参数（带 bound）
    let impl_gen = if parts.impl_generics.is_empty() {
        quote!()
    } else {
        let params = parts.impl_generics.iter().map(|(name, bound)| {
            match bound {
                Some(b) => {
                    let b_tokens = b.to_token_stream();
                    quote!(#name: #b_tokens)
                }
                None => name.clone(),
            }
        }).collect::<Vec<_>>();
        quote!(<#(#params),*>)
    };

    // trait 泛型参数（仅名字）
    let trait_gen = if parts.trait_generic_names.is_empty() {
        quote!()
    } else {
        let names = &parts.trait_generic_names;
        quote!(<#(#names),*>)
    };

    // 目标类型
    let target = &parts.target_type;

    // impl body：关联类型 + 用户 body
    let mut body_tokens: Vec<TokenStream> = Vec::new();
    for (name, value) in &parts.associated_types {
        body_tokens.push(quote!(type #name = #value;));
    }
    if let Some(body) = &parts.body {
        body_tokens.push(body.clone());
    }

    // 属性
    let attrs = parts.attrs;

    quote! {
        #(#attrs)*
        #unsafe_kw impl #impl_gen #trait_name #trait_gen for #target {
            #(#body_tokens)*
        }
    }
}
