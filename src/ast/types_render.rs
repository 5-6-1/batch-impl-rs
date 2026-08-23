use crate::ast::*;
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

pub(crate) fn params_to_tokens(base: &TokenStream, tp: &TyTypeParam) -> TokenStream {
    let mut all = tp.params.iter().map(|(name, _)| name.to_token_stream()).collect::<Vec<_>>();
    for (name, value) in &tp.bindings {
        all.push(quote!(#name = #value));
    }
    if all.is_empty() {
        // render only base when both params and bindings are empty
        return base.clone();
    }
    quote!(#base < #(#all),* >)
}

/// Renders a single generic declaration: `name: bound` (with bound) or bare `name`.
/// This file's `TyTypeParam` rendering is also reused by codegen's impl generics.
pub(crate) fn render_param(name: &impl ToTokens, bound: Option<&Ty>) -> TokenStream {
    match bound {
        Some(b) => {
            let b_tokens = b.to_token_stream();
            quote!(#name: #b_tokens)
        }
        None => name.to_token_stream(),
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
        // render empty when both params and bindings are empty
        return quote!();
    }
    quote!(<#(#all),*>)
}

/// Two-state rendering with optional inner: `Some(inner)` concatenates inner and
/// payload (order decided by `inner_first`), `None` renders the bare payload.
/// The WithPrefix/WithAttr/WithCode/WithWhere arms are isomorphic and all
/// converge here.
fn render_optional(inner: Option<&Ty>, payload: TokenStream, inner_first: bool) -> TokenStream {
    match inner {
        Some(i) => {
            let inner = i.to_token_stream();
            if inner_first { quote!(#inner #payload) } else { quote!(#payload #inner) }
        }
        None => payload,
    }
}

impl ToTokens for Ty {
    fn to_tokens(&self, out: &mut TokenStream) {
        out.extend(match &self.kind {
            TyKind::Primitive(p) => p.0.clone(),
            TyKind::Generic(g) => params_to_tokens(&g.0.to_token_stream(), &g.1),
            TyKind::Trait(t) => params_to_tokens(&t.0, &t.1),
            TyKind::Array(a) => {
                let elems = a.0.iter().map(|e| e.to_token_stream()).collect::<Vec<_>>();
                quote!([#(#elems),*])
            }
            TyKind::Tuple(t) => {
                let elems = t.0.iter().map(|e| e.to_token_stream()).collect::<Vec<_>>();
                quote!((#(#elems,)*))
            }
            // Splats are never expanded at parse/apply/expand time (splat
            // survival); they render with their marker so the codegen
            // postprocess (`expand_splats`) can spot and expand them —
            // `*(A,B)` stays `*(A,B)`, `*[A,B]` stays `*[A,B]`.
            TyKind::Splat(s) => {
                let elems = s.elems().iter().map(|e| e.to_token_stream()).collect::<Vec<_>>();
                match s {
                    TySplat::Array(_) => quote!(*[#(#elems),*]),
                    TySplat::Tuple(_) => quote!(*(#(#elems),*)),
                }
            }
            TyKind::Group(g) => {
                let inner = g.0.to_token_stream();
                quote!((#inner))
            }
            TyKind::PrimitiveArray(pa) => match (&pa.0, &pa.1) {
                (Some(elem), None) => {
                    let inner = elem.to_token_stream();
                    quote!([#inner])
                }
                (Some(elem), Some(size)) => {
                    let inner = elem.to_token_stream();
                    quote!([#inner; #size])
                }
                // empty base `[]` is not a valid type; render defensively
                (None, _) => quote!([]),
            },
            TyKind::WithPrefix(wp) => render_optional(wp.1.as_deref(), prefix_token(wp.0), false),
            TyKind::WithDyn(wd) => {
                let inner = wd.0.to_token_stream();
                let mut ts = quote!(dyn #inner);
                for b in &wd.1 {
                    ts.extend(b.clone());
                }
                ts
            }
            TyKind::WithFor(wf) => {
                let inner = wf.1.to_token_stream();
                let binder = &wf.0;
                quote!(for < #binder > #inner)
            }
            TyKind::Fn(f) => {
                let u = f.2.then_some(quote!(unsafe));
                let head = match f.3 {
                    crate::ast::FnKind::Bare => quote!(fn),
                    crate::ast::FnKind::Trait => quote!(Fn),
                    crate::ast::FnKind::TraitMut => quote!(FnMut),
                    crate::ast::FnKind::TraitOnce => quote!(FnOnce),
                };
                match &f.0 {
                    Some(params) => {
                        let params = params.iter().map(|p| p.to_token_stream()).collect::<Vec<_>>();
                        match &f.1 {
                            Some(ret) => {
                                let ret_tokens = ret.to_token_stream();
                                quote!(#u #head(#(#params),*) -> #ret_tokens)
                            }
                            None => quote!(#u #head(#(#params),*)),
                        }
                    }
                    None => quote!(#u #head),
                }
            }
            TyKind::TypeParam(tp) => params_to_tokens_no_base(tp),
            TyKind::WithAttr(w) => {
                let stream = &w.0.0;
                render_optional(w.1.as_deref(), quote!(#[#stream]), false)
            }
            TyKind::Num(n) => {
                let lit = proc_macro2::Literal::usize_unsuffixed(n.0);
                quote!(#lit)
            }
            TyKind::Range(r) => {
                let start = proc_macro2::Literal::usize_unsuffixed(r.start);
                let end = proc_macro2::Literal::usize_unsuffixed(r.end);
                if r.inclusive { quote!(#start ..= #end) } else { quote!(#start .. #end) }
            }
            TyKind::BoundList(b) => {
                let elems = b.0.iter().map(|e| e.to_token_stream()).collect::<Vec<_>>();
                quote!(#(#elems)+*)
            }
            TyKind::WithTrait(wt) => {
                let trait_tokens = params_to_tokens(&wt.0.0, &wt.0.1);
                let inner = wt.1.to_token_stream();
                quote!(#trait_tokens #inner)
            }
            TyKind::WithType(wt) => {
                let tp_tokens = params_to_tokens_no_base(&wt.0);
                let inner = wt.1.to_token_stream();
                quote!(#tp_tokens #inner)
            }
            TyKind::WithCode(wc) => {
                let stream = &wc.1.0;
                render_optional(wc.0.as_deref(), quote!({#stream}), true)
            }
            TyKind::WithWhere(ww) => {
                let stream = &ww.1.0;
                render_optional(ww.0.as_deref(), quote!(where #stream), true)
            }
            // The `impl{...}` template is consumed by the codegen shape match
            // — never emitted; render only the inner type (bare `None` = empty).
            TyKind::WithImpl(wi) => match &wi.0 {
                Some(inner) => inner.to_token_stream(),
                None => quote!(),
            },
            TyKind::Error(e) => e.0.clone(),
        })
    }
}

/// Renders prefix modifier keywords (`&`/`&mut`/`*const`/`*mut`/`self`/`unsafe`)
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
