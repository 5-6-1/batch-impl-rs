use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::Ident;
use std::cell::Cell;

#[derive(Clone)]
/// `[...,]`
pub(crate) struct TyArray(pub(crate) Vec<Ty>);
#[derive(Clone)]
/// `(...,)`
pub(crate) struct TyTuple(pub(crate) Vec<Ty>);
#[derive(Clone)]
/// `(...)`
pub(crate) struct TyGroup(pub(crate) Box<Ty>);
#[derive(Clone)]
/// `[...]`
pub(crate) struct TySlice(pub(crate) Box<Ty>);
#[derive(Clone)]
/// `[...;...]`
pub(crate) struct TyFixedArray(pub(crate) Box<Ty>, pub(crate) TokenStream);
#[derive(Clone)]
/// `ident`
pub(crate) struct TyPrimitive(pub(crate) TokenStream);
#[derive(Clone)]
/// `T<...>`
pub(crate) struct TyGeneric(pub(crate) Box<Ty>, pub(crate) TyTypeParam);

#[derive(Clone)]
/// `trait-name<...>`
pub(crate) struct TyTrait(pub(crate) TokenStream, pub(crate) TyTypeParam);
/// `<T: Clone, U, Item=V>` 泛型参数列表：positional 参数（可带 bound）+ 关联类型绑定
#[derive(Clone)]
pub(crate) struct TyTypeParam {
    pub(crate) params: Vec<(TokenStream, Option<Ty>)>,
    pub(crate) bindings: Vec<(TokenStream, TokenStream)>,
}

impl TyTypeParam {
    /// 构造单个无 bound 参数（`T^U` 中 `U` 变为 `<U>`）
    pub(crate) fn single(arg: &Ty) -> Self {
        TyTypeParam { params: vec![(arg.to_token_stream(), None)], bindings: vec![] }
    }

    /// 追加一个无 bound 参数（`T<A>^B` 中 `B` 追加到 `<A,B>`）
    pub(crate) fn push_arg(&mut self, arg: &Ty) {
        self.params.push((arg.to_token_stream(), None));
    }

    /// 合并另一个参数列表（`T<A>^<B,C>` 中 `<B,C>` 的 params + bindings 合并进来）
    pub(crate) fn extend(&mut self, other: TyTypeParam) {
        self.params.extend(other.params);
        self.bindings.extend(other.bindings);
    }
}
#[derive(Clone)]
/// `{...}` — 附着在类型上的代码块
pub(crate) struct TyCodeBlock(pub(crate) TokenStream);
#[derive(Clone)]
/// `T { code }` — 类型 + 代码块
pub(crate) struct TyWithCode(pub(crate) Box<Ty>, pub(crate) TokenStream);
#[derive(Copy, Clone)]
/// `& &mut *const *mut fn self unsafe`
pub(crate) enum TyPrefix {
    Ref,
    RefMut,
    PtrConst,
    PtrMut,
    SelfType,
    Fn,
    Unsafe,
}
#[derive(Clone)]
/// prefix `T`
pub(crate) struct TyModified(pub(crate) TyPrefix, pub(crate) Box<Ty>);
#[derive(Clone)]
/// `fn(...)->T`
pub(crate) struct TyFn(pub(crate) Vec<Ty>, pub(crate) Option<Box<Ty>>);
#[derive(Clone)]
/// `unsafe T`
pub(crate) struct TyUnsafe(pub(crate) Box<Ty>);
#[derive(Clone)]
/// `#[...]`
pub(crate) struct TyAttr(pub(crate) TokenStream);
#[derive(Clone)]
/// `#[...] T`
pub(crate) struct TyWithAttr(pub(crate) TyAttr, pub(crate) Box<Ty>);
#[derive(Copy, Clone)]
/// `N`
pub(crate) struct TyNum(pub(crate) u8);
#[derive(Copy, Clone)]
/// `N..M` `N..=M`
pub(crate) struct TyRange { pub(crate) start: u8, pub(crate) end: u8, pub(crate) inclusive: bool }
#[derive(Clone)]
/// `trait-name<...> T` — trait name applied to non-TypeParam right
pub(crate) struct TyWithTrait(pub(crate) TyTrait, pub(crate) Box<Ty>);
#[derive(Clone)]
/// `<T...> T` — type param applied to non-TypeParam right
pub(crate) struct TyWithType(pub(crate) TyTypeParam, pub(crate) Box<Ty>);

/// DSL 解析输出的类型表达式 AST。
///
/// 节点分三类：
/// - **叶子**（Primitive / Num / Range）：不可再展开的原子
/// - **包装**（WithType / WithTrait / WithCode / WithAttr / Unsafe / Modified）：携带元数据，在 codegen 阶段被拆解
/// - **容器**（Array / Tuple / Group / Slice / FixedArray）：可展开为多个叶子的集合
#[derive(Clone)]
pub(crate) enum Ty {
    Array(TyArray),
    Tuple(TyTuple),
    Group(TyGroup),
    Slice(TySlice),
    FixedArray(TyFixedArray),
    Primitive(TyPrimitive),
    Generic(TyGeneric),
    Trait(TyTrait),
    TypeParam(TyTypeParam),
    CodeBlock(TyCodeBlock),
    Prefix(TyPrefix),
    Modified(TyModified),
    Fn(TyFn),
    Unsafe(TyUnsafe),
    Attr(TyAttr),
    WithAttr(TyWithAttr),
    WithTrait(TyWithTrait),
    WithType(TyWithType),
    WithCode(TyWithCode),
    Num(TyNum),
    Range(TyRange),
}
fn params_to_tokens(base: &TokenStream, tp: &TyTypeParam) -> TokenStream {
    let mut all = tp.params.iter()
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();
    for (name, value) in &tp.bindings {
        all.push(quote!(#name = #value));
    }
    quote!(#base < #(#all),* >)
}

fn params_to_tokens_no_base(tp: &TyTypeParam) -> TokenStream {
    let mut all = vec![];
    for (name, bound) in &tp.params {
        match bound {
            Some(b) => {
                let b_tokens = b.to_token_stream();
                all.push(quote!(#name: #b_tokens));
            }
            None => all.push(name.clone()),
        }
    }
    for (name, value) in &tp.bindings {
        all.push(quote!(#name = #value));
    }
    quote!(<#(#all),*>)
}

impl Ty {
    /// 展开并列列表类节点：Array 直接拆包，WithCode/Group 透传后递归。
    /// 不可展开的叶子原样经 `Err` 返回（由调用方决定是收集还是继续展开）。
    pub(crate) fn expand(self) -> Result<Vec<Ty>, Ty> {
        match self {
            Ty::Array(ty) => Ok(ty.0),
            Ty::WithCode(wc) => match (*wc.0).expand() {
                Ok(expanded) => Ok(expanded
                    .into_iter()
                    .map(|inner| Ty::WithCode(TyWithCode(Box::new(inner), wc.1.clone())))
                    .collect()),
                Err(leaf) => Err(Ty::WithCode(TyWithCode(Box::new(leaf), wc.1))),
            },
            Ty::Group(g) => (*g.0).expand(),
            other => Err(other),
        }
    }
}

impl ToTokens for Ty {
    fn to_tokens(&self, out: &mut TokenStream) {
        out.extend(match self {
            Ty::Primitive(p) => p.0.clone(),
            Ty::Generic(g) => params_to_tokens(&g.0.to_token_stream(), &g.1),
            Ty::Trait(t) => params_to_tokens(&t.0, &t.1),
            Ty::Array(a) => {
                let elems = a.0.iter().map(|e| e.to_token_stream()).collect::<Vec<_>>();
                quote!([#(#elems),*])
            }
            Ty::Tuple(t) => {
                let elems = t.0.iter().map(|e| e.to_token_stream()).collect::<Vec<_>>();
                quote!((#(#elems,)*))
            }
            Ty::Group(g) => {
                let inner = g.0.to_token_stream();
                quote!((#inner))
            }
            Ty::Slice(s) => {
                let inner = s.0.to_token_stream();
                quote!([#inner])
            }
            Ty::FixedArray(f) => {
                let inner = f.0.to_token_stream();
                let size = &f.1;
                quote!([#inner; #size])
            }
            Ty::Modified(m) => {
                let prefix_tokens = match m.0 {
                    TyPrefix::Ref => quote!(&),
                    TyPrefix::RefMut => quote!(&mut),
                    TyPrefix::PtrConst => quote!(*const),
                    TyPrefix::PtrMut => quote!(*mut),
                    _ => unreachable!(),
                };
                let inner = m.1.to_token_stream();
                quote!(#prefix_tokens #inner)
            }
            Ty::Fn(f) => {
                let params = f.0.iter().map(|p| p.to_token_stream()).collect::<Vec<_>>();
                match &f.1 {
                    Some(ret) => {
                        let ret_tokens = ret.to_token_stream();
                        quote!(fn(#(#params),*) -> #ret_tokens)
                    }
                    None => quote!(fn(#(#params),*)),
                }
            }
            Ty::TypeParam(tp) => params_to_tokens_no_base(tp),
            Ty::Unsafe(u) => {
                let inner = u.0.to_token_stream();
                quote!(unsafe #inner)
            }
            Ty::Attr(a) => {
                let stream = &a.0;
                quote!(#[#stream])
            }
            Ty::WithAttr(w) => {
                let stream = &w.0 .0;
                let inner = w.1.to_token_stream();
                quote!(#[#stream] #inner)
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
            Ty::CodeBlock(b) => {
                let stream = &b.0;
                quote!({#stream})
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
                let inner = wc.0.to_token_stream();
                let stream = &wc.1;
                quote!(#inner {#stream})
            }
            Ty::Prefix(p) => match p {
                TyPrefix::Ref => quote![&],
                TyPrefix::RefMut => quote![&mut],
                TyPrefix::PtrConst => quote![*const],
                TyPrefix::PtrMut => quote![*mut],
                TyPrefix::SelfType => quote![self],
                TyPrefix::Fn => quote![fn],
                TyPrefix::Unsafe => quote![unsafe],
            },
        })
    }
}

macro_rules! impl_from_for_ty {
    ($($struct:ident => $variant:ident),* $(,)?) => {
        $(
            impl From<$struct> for Ty {
                fn from(value: $struct) -> Self {
                    Ty::$variant(value)
                }
            }
            impl From<$struct> for Box<Ty> {
                fn from(value: $struct) -> Self {
                    Box::new(Ty::$variant(value))
                }
            }
        )*
    };
}

impl_from_for_ty! {
    TyArray => Array,
    TyTuple => Tuple,
    TyGroup => Group,
    TySlice => Slice,
    TyFixedArray => FixedArray,
    TyPrimitive => Primitive,
    TyGeneric => Generic,
    TyTrait => Trait,
    TyTypeParam => TypeParam,
    TyCodeBlock => CodeBlock,
    TyPrefix => Prefix,
    TyModified => Modified,
    TyFn => Fn,
    TyUnsafe => Unsafe,
    TyAttr => Attr,
    TyWithAttr => WithAttr,
    TyWithTrait => WithTrait,
    TyWithType => WithType,
    TyWithCode => WithCode,
    TyNum => Num,
    TyRange => Range,
}

/// 运算符优先级层级（从低到高：`;` < `,` < `-` < `^`，`Prim` 为无运算符的原子级）。
///
/// 每个层级定义一组"停止字符"：`parse_operand` 在该层级扫描时遇到这些字符就截断，
/// 然后把截出的切片交给更高优先级递归解析。
#[derive(Copy, Clone)]
pub(crate) enum Op {
    Semi,
    Comma,
    Dash,
    Caret,
    Prim,
}

impl Op {
    /// 更高一级的优先级
    pub(crate) fn next(self) -> Option<Op> {
        match self {
            Op::Semi => Some(Op::Comma),
            Op::Comma => Some(Op::Dash),
            Op::Dash => Some(Op::Caret),
            Op::Caret => Some(Op::Prim),
            Op::Prim => None,
        }
    }

    /// 该优先级下会截断操作数的字符
    pub(crate) fn stop_chars(self) -> &'static [char] {
        match self {
            // Semi 同时停在 `,`：项边界与段落边界都由它截出，交给调用方区分
            Op::Semi => &[',', ';'],
            Op::Comma => &[','],
            Op::Dash => &['-', ','],
            Op::Caret => &['^', '-', ','],
            Op::Prim => &[],
        }
    }
}

thread_local! {
    static FRESH_COUNTER: Cell<usize> = 0.into();
}

/// 重置 fresh 参数计数器（每个宏入口调用一次，确保生成的泛型名不跨宏冲突）
pub(crate) fn reset_fresh_counter() {
    FRESH_COUNTER.set(0);
}

/// 生成一个不与用户代码冲突的全新泛型参数名（`_Param_0_BatchGen_`、`_Param_1_BatchGen_` ……）
pub(crate) fn fresh_param() -> TokenStream {
    FRESH_COUNTER.with(|c| {
        let n = c.get();
        c.set(n + 1);
        let name = format!("_Param_{}_BatchGen_", n);
        let ident = Ident::new(&name, proc_macro2::Span::call_site());
        quote!(#ident)
    })
}
