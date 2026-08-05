use crate::ast::*;
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

pub(crate) fn params_to_tokens(base: &TokenStream, tp: &TyTypeParam) -> TokenStream {
    let mut all = tp.params.iter().map(|(name, _)| name.clone()).collect::<Vec<_>>();
    for (name, value) in &tp.bindings {
        all.push(quote!(#name = #value));
    }
    if all.is_empty() {
        // params + bindings 都空时只渲染 base
        return base.clone();
    }
    quote!(#base < #(#all),* >)
}

/// 渲染单条泛型声明：`name: bound`（有 bound）或裸 `name`。
/// TyTypeParam 渲染（本文件）与 codegen 的 impl 泛型复用。
pub(crate) fn render_param(name: &TokenStream, bound: Option<&Ty>) -> TokenStream {
    match bound {
        Some(b) => {
            let b_tokens = b.to_token_stream();
            quote!(#name: #b_tokens)
        }
        None => name.clone(),
    }
}

pub(crate) fn params_to_tokens_no_base(tp: &TyTypeParam) -> TokenStream {
    let mut all = vec![];
    for (name, bound) in &tp.params {
        all.push(render_param(name, bound.as_ref()));
    }
    for (name, value) in &tp.bindings {
        all.push(quote!(#name = #value));
    }
    if all.is_empty() {
        // params + bindings 都空时渲染为空
        return quote!();
    }
    quote!(<#(#all),*>)
}

/// 可选内层双态渲染：`Some(inner)` 时 inner 与 payload 拼接（顺序由
/// `inner_first` 定），`None` 时裸 payload。WithPrefix/WithAttr/WithCode/
/// WithWhere 四臂同构，收敛于此。
fn render_optional(
    inner: Option<&Ty>, payload: TokenStream, inner_first: bool,
) -> TokenStream {
    match inner {
        Some(i) => {
            let inner = i.to_token_stream();
            if inner_first {
                quote!(#inner #payload)
            } else {
                quote!(#payload #inner)
            }
        }
        None => payload,
    }
}

impl ToTokens for Ty {
    fn to_tokens(&self, out: &mut TokenStream) {
        out.extend(match self {
            Ty::Primitive(p) => p.0.clone(),
            Ty::Generic(g) => params_to_tokens(&g.0.to_token_stream(), &g.1),
            Ty::Trait(t) => params_to_tokens(&t.0, &t.1),
            Ty::Array(a) => {
                let elems =
                    a.0.iter().map(|e| e.to_token_stream()).collect::<Vec<_>>();
                quote!([#(#elems),*])
            }
            Ty::Tuple(t) => {
                let elems =
                    t.0.iter().map(|e| e.to_token_stream()).collect::<Vec<_>>();
                quote!((#(#elems,)*))
            }
            Ty::Group(g) => {
                let inner = g.0.to_token_stream();
                quote!((#inner))
            }
            Ty::PrimitiveArray(pa) => match (&pa.0, &pa.1) {
                (Some(elem), None) => {
                    let inner = elem.to_token_stream();
                    quote!([#inner])
                }
                (Some(elem), Some(size)) => {
                    let inner = elem.to_token_stream();
                    quote!([#inner; #size])
                }
                // 空基座 `[]` 不是有效类型，防御性渲染
                (None, _) => quote!([]),
            },
            Ty::WithPrefix(wp) => {
                render_optional(wp.1.as_deref(), prefix_token(wp.0), false)
            }
            Ty::Fn(f) => {
                let u = f.2.then_some(quote!(unsafe));
                match &f.0 {
                    Some(params) => {
                        let params = params
                            .iter()
                            .map(|p| p.to_token_stream())
                            .collect::<Vec<_>>();
                        match &f.1 {
                            Some(ret) => {
                                let ret_tokens = ret.to_token_stream();
                                quote!(#u fn(#(#params),*) -> #ret_tokens)
                            }
                            None => quote!(#u fn(#(#params),*)),
                        }
                    }
                    None => quote!(#u fn),
                }
            }
            Ty::TypeParam(tp) => params_to_tokens_no_base(tp),
            Ty::WithAttr(w) => {
                let stream = &w.0.0;
                render_optional(w.1.as_deref(), quote!(#[#stream]), false)
            }
            Ty::Num(n) => {
                let n = n.0;
                quote!(#n)
            }
            Ty::Range(r) => {
                let start = r.start;
                let end = r.end;
                if r.inclusive {
                    quote!(#start ..= #end)
                } else {
                    quote!(#start .. #end)
                }
            }
            Ty::WithTrait(wt) => {
                let trait_tokens = params_to_tokens(&wt.0.0, &wt.0.1);
                let inner = wt.1.to_token_stream();
                quote!(#trait_tokens #inner)
            }
            Ty::WithType(wt) => {
                let tp_tokens = params_to_tokens_no_base(&wt.0);
                let inner = wt.1.to_token_stream();
                quote!(#tp_tokens #inner)
            }
            Ty::WithCode(wc) => {
                let stream = &wc.1.0;
                render_optional(wc.0.as_deref(), quote!({#stream}), true)
            }
            Ty::WithWhere(ww) => {
                let stream = &ww.1.0;
                render_optional(ww.0.as_deref(), quote!(where #stream), true)
            }
            Ty::Error(e) => e.0.clone(),
        })
    }
}

/// 渲染前缀修饰符关键字（`&`/`&mut`/`*const`/`*mut`/`self`/`unsafe`）
fn prefix_token(prefix: TyPrefix) -> TokenStream {
    match prefix {
        TyPrefix::Ref => quote!(&),
        TyPrefix::RefMut => quote!(&mut),
        TyPrefix::PtrConst => quote!(*const),
        TyPrefix::PtrMut => quote!(*mut),
        TyPrefix::SelfType => quote!(self),
        TyPrefix::Unsafe => quote!(unsafe),
    }
}
