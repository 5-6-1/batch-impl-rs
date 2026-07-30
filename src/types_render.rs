use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use crate::types::*;

pub(crate) fn params_to_tokens(base: &TokenStream, tp: &TyTypeParam) -> TokenStream {
    let mut all = tp
        .params
        .iter()
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();
    for (name, value) in &tp.bindings {
        all.push(quote!(#name = #value));
    }
    if all.is_empty() {
        // params + bindings 都空时只渲染 base；where 不进类型表达式
        // （由 codegen 提取）。
        return base.clone();
    }
    quote!(#base < #(#all),* >)
}

pub(crate) fn params_to_tokens_no_base(tp: &TyTypeParam) -> TokenStream {
    let mut all = vec![];
    for (name, bound) in &tp.params {
        match bound {
            Some(b) => {
                let b_tokens = b.to_token_stream();
                all.push(quote!(#name: #b_tokens));
            },
            None => all.push(name.clone()),
        }
    }
    for (name, value) in &tp.bindings {
        all.push(quote!(#name = #value));
    }
    if all.is_empty() {
        // 仅 where 的 TypeParam 在类型表达式中渲染为空
        // （where 由 codegen 提取）。
        return quote!();
    }
    quote!(<#(#all),*>)
}

impl ToTokens for Ty {
    fn to_tokens(&self, out: &mut TokenStream) {
        out.extend(match self {
            Ty::Primitive(p) => p.0.clone(),
            Ty::Generic(g) => {
                params_to_tokens(&g.0.to_token_stream(), &g.1)
            },
            Ty::Trait(t) => params_to_tokens(&t.0, &t.1),
            Ty::Array(a) => {
                let elems =
                    a.0.iter()
                        .map(|e| e.to_token_stream())
                        .collect::<Vec<_>>();
                quote!([#(#elems),*])
            },
            Ty::Tuple(t) => {
                let elems =
                    t.0.iter()
                        .map(|e| e.to_token_stream())
                        .collect::<Vec<_>>();
                quote!((#(#elems,)*))
            },
            Ty::Group(g) => {
                let inner = g.0.to_token_stream();
                quote!((#inner))
            },
            Ty::Slice(s) => {
                let inner = s.0.to_token_stream();
                quote!([#inner])
            },
            Ty::FixedArray(f) => {
                let inner = f.0.to_token_stream();
                let size = &f.1;
                quote!([#inner; #size])
            },
            Ty::Modified(m) => {
                let prefix_tokens = match m.0 {
                    TyPrefix::Ref => quote!(&),
                    TyPrefix::RefMut => quote!(&mut),
                    TyPrefix::PtrConst => quote!(*const),
                    TyPrefix::PtrMut => quote!(*mut),
                    _ => quote!(compile_error!(
                        "batch-impl: 内部错误：TyModified 含有非引用前缀"
                    )),
                };
                let inner = m.1.to_token_stream();
                quote!(#prefix_tokens #inner)
            },
            Ty::Fn(f) => {
                let params =
                    f.0.iter()
                        .map(|p| p.to_token_stream())
                        .collect::<Vec<_>>();
                match &f.1 {
                    Some(ret) => {
                        let ret_tokens = ret.to_token_stream();
                        quote!(fn(#(#params),*) -> #ret_tokens)
                    },
                    None => quote!(fn(#(#params),*)),
                }
            },
            Ty::TypeParam(tp) => params_to_tokens_no_base(tp),
            Ty::Unsafe(u) => {
                let inner = u.0.to_token_stream();
                quote!(unsafe #inner)
            },
            Ty::Attr(a) => {
                let stream = &a.0;
                quote!(#[#stream])
            },
            Ty::WithAttr(w) => {
                let stream = &w.0.0;
                let inner = w.1.to_token_stream();
                quote!(#[#stream] #inner)
            },
            Ty::Num(n) => {
                let n = n.0;
                quote!(#n)
            },
            Ty::Range(r) => {
                let start = r.start;
                let end = r.end;
                if r.inclusive {
                    quote!(#start ..= #end)
                } else {
                    quote!(#start .. #end)
                }
            },
            Ty::CodeBlock(b) => {
                let stream = &b.0;
                quote!({#stream})
            },
            Ty::WithTrait(wt) => {
                let trait_tokens = params_to_tokens(&wt.0.0, &wt.0.1);
                let inner = wt.1.to_token_stream();
                quote!(#trait_tokens #inner)
            },
            Ty::WithType(wt) => {
                let tp_tokens = params_to_tokens_no_base(&wt.0);
                let inner = wt.1.to_token_stream();
                quote!(#tp_tokens #inner)
            },
            Ty::WithCode(wc) => {
                let inner = wc.0.to_token_stream();
                let stream = &wc.1.0;
                quote!(#inner {#stream})
            },
            Ty::Prefix(p) => match p {
                TyPrefix::Ref => quote![&],
                TyPrefix::RefMut => quote![&mut],
                TyPrefix::PtrConst => quote![*const],
                TyPrefix::PtrMut => quote![*mut],
                TyPrefix::SelfType => quote![self],
                TyPrefix::Fn => quote![fn],
                TyPrefix::Unsafe => quote![unsafe],
            },
            Ty::Where(w) => {
                let stream = &w.0;
                quote!(where{#stream})
            },
            Ty::WithWhere(ww) => {
                let inner = ww.0.to_token_stream();
                let stream = &ww.1.0;
                quote!(#inner where{#stream})
            },
            Ty::Error(e) => e.0.clone(),
        })
    }
}
