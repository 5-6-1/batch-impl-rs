use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use std::cell::Cell;
use syn::Ident;

#[derive(Clone, Debug)]
/// `[...,]`
pub(crate) struct TyArray(pub(crate) Vec<Ty>);
#[derive(Clone, Debug)]
/// `(...,)`
pub(crate) struct TyTuple(pub(crate) Vec<Ty>);
#[derive(Clone, Debug)]
/// `(...)`
pub(crate) struct TyGroup(pub(crate) Box<Ty>);
#[derive(Clone, Debug)]
/// `[]`（种子）/ `[T]`（切片）/ `[T; N]`（定长数组）— 元素 `None` 表示空 `[]`，长度 `None` 表示切片
pub(crate) struct TyPrimitiveArray(
    pub(crate) Option<Box<Ty>>,
    pub(crate) Option<TokenStream>,
);
#[derive(Clone, Debug)]
/// `ident`
pub(crate) struct TyPrimitive(pub(crate) TokenStream);
#[derive(Clone, Debug)]
/// `T<...>`
pub(crate) struct TyGeneric(pub(crate) Box<Ty>, pub(crate) TyTypeParam);

#[derive(Clone, Debug)]
/// `trait-name<...>`
pub(crate) struct TyTrait(pub(crate) TokenStream, pub(crate) TyTypeParam);
/// `<T: Clone, U, Item=V>` 泛型参数列表：positional 参数（可带 bound）+
/// 关联类型绑定。
#[derive(Clone, Debug)]
pub(crate) struct TyTypeParam {
    pub(crate) params: Vec<(TokenStream, Option<Ty>)>,
    pub(crate) bindings: Vec<(TokenStream, TokenStream)>,
}

impl TyTypeParam {
    /// 构造单个无 bound 参数（`T^U` 中 `U` 变为 `<U>`）
    pub(crate) fn single(arg: &Ty) -> Self {
        TyTypeParam {
            params: vec![(arg.to_token_stream(), None)],
            bindings: vec![],
        }
    }

    /// 追加一个无 bound 参数（`T<A>^B` 中 `B` 追加到 `<A,B>`）
    pub(crate) fn push_arg(&mut self, arg: &Ty) {
        self.params.push((arg.to_token_stream(), None));
    }

    /// 合并另一个参数列表（`T<A>^<B,C>` 中 `<B,C>` 的
    /// params + bindings 合并进来）
    pub(crate) fn extend(&mut self, other: TyTypeParam) {
        self.params.extend(other.params);
        self.bindings.extend(other.bindings);
    }
}
#[derive(Clone, Debug)]
/// `{...}` — 附着在类型上的代码块
pub(crate) struct TyCodeBlock(pub(crate) TokenStream);
#[derive(Clone, Debug)]
/// `{...}`（裸）或 `T { code }` — 内层 `None` 表示裸代码块。
/// 裸代码块在 codegen 阶段**原样作为顶层 item 注入**输出（仅服务于"指令
/// 独立成整个 spec"的退化形态：开放指令 `#name(args){body}` 展开的
/// `{name!{...}}` 块附着到类型时是普通 impl body，独立时经此路径顶层输出）。
pub(crate) struct TyWithCode(pub(crate) Option<Box<Ty>>, pub(crate) TyCodeBlock);
#[derive(Copy, Clone, Debug)]
/// `& &mut *const *mut self unsafe` — 类型前缀修饰符
pub(crate) enum TyPrefix {
    Ref,
    RefMut,
    PtrConst,
    PtrMut,
    SelfType,
    Unsafe,
}

#[derive(Clone, Debug)]
/// 裸前缀（`&`/`unsafe` 等）或 `prefix T` — 内层 `None` 表示裸前缀
pub(crate) struct TyWithPrefix(pub(crate) TyPrefix, pub(crate) Option<Box<Ty>>);
#[derive(Clone, Debug)]
/// 裸 `fn` / `fn(...)` / `fn(...)->T` — 参数 `None` 表示尚未填入；
/// 第三字段 `is_unsafe`：`unsafe fn(...)` 类型的标记（`unsafe` 修饰 fn 类型本身，
/// 区别于 `unsafe^T` 的 unsafe impl 标记）
pub(crate) struct TyFn(
    pub(crate) Option<Vec<Ty>>,
    pub(crate) Option<Box<Ty>>,
    pub(crate) bool,
);
#[derive(Clone, Debug)]
/// `#[...]` — 属性本身
pub(crate) struct TyAttr(pub(crate) TokenStream);
#[derive(Clone, Debug)]
/// `#[...]`（裸）或 `#[...] T` — 内层 `None` 表示裸属性
pub(crate) struct TyWithAttr(pub(crate) TyAttr, pub(crate) Option<Box<Ty>>);
#[derive(Copy, Clone, Debug)]
/// `N`
pub(crate) struct TyNum(pub(crate) usize);
#[derive(Copy, Clone, Debug)]
/// `N..M` `N..=M`
pub(crate) struct TyRange {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) inclusive: bool,
}
#[derive(Clone, Debug)]
/// `trait-name<...> T` — trait name applied to non-TypeParam right
pub(crate) struct TyWithTrait(pub(crate) TyTrait, pub(crate) Box<Ty>);
#[derive(Clone, Debug)]
/// `<T...> T` — type param applied to non-TypeParam right
pub(crate) struct TyWithType(pub(crate) TyTypeParam, pub(crate) Box<Ty>);
#[derive(Clone, Debug)]
/// 编译期错误信号 — 当 DSL 语义不合法时产生，最终输出 `compile_error!`
pub(crate) struct TyError(pub(crate) TokenStream);

#[derive(Clone, Debug)]
pub(crate) struct TyWhere(pub(crate) TokenStream);

#[derive(Clone, Debug)]
/// 裸 `where{...}` 或 `T where{...}` — 内层 `None` 表示裸 where 后缀
pub(crate) struct TyWithWhere(pub(crate) Option<Box<Ty>>, pub(crate) TyWhere);

/// DSL 解析输出的类型表达式 AST。
///
/// 节点分三类：
/// - **叶子**（Primitive / Num / Range）：不可再展开的原子
/// - **包装**（WithType / WithTrait / WithPrefix / WithCode / WithWhere / WithAttr / Fn）：携带元数据，在 codegen 阶段被拆解
/// - **容器**（Array / Tuple / Group / PrimitiveArray）：可展开为多个叶子的集合
///
/// 前缀/后缀类包装（WithPrefix / WithCode / WithAttr / WithWhere / Fn）的内层用
/// `Option<Box<Ty>>` 表示"暂未附着类型"的裸状态，避免枚举中再存半成品变体。
#[derive(Clone, Debug)]
pub(crate) enum Ty {
    Array(TyArray),
    Tuple(TyTuple),
    Group(TyGroup),
    PrimitiveArray(TyPrimitiveArray),
    Primitive(TyPrimitive),
    Generic(TyGeneric),
    Trait(TyTrait),
    TypeParam(TyTypeParam),
    Fn(TyFn),
    WithPrefix(TyWithPrefix),
    WithAttr(TyWithAttr),
    WithTrait(TyWithTrait),
    WithType(TyWithType),
    WithCode(TyWithCode),
    WithWhere(TyWithWhere),
    Num(TyNum),
    Range(TyRange),
    Error(TyError),
}
/// [`Ty::expand`] 的结果：`Leaf` = 不可再展开的叶子；`Many` = 展开为多个节点。
pub(crate) enum Expand {
    Leaf(Ty),
    Many(Vec<Ty>),
}

/// 包装变体的公共"递归内层并重包"逻辑：`make` 由内层重建包装；
/// `inner` 为 `None`（裸包装）时经 `make(None)` 原样交还（叶子）。
/// 供 `Ty::expand` 的 WithCode/WithWhere/WithAttr/WithPrefix 臂复用。
fn expand_wrapped<F>(make: F, inner: Option<Box<Ty>>) -> Expand
where
    F: Fn(Option<Box<Ty>>) -> Ty,
{
    match inner {
        Some(i) => match i.expand() {
            Expand::Many(v) => {
                Expand::Many(v.into_iter().map(|e| make(Some(e.into()))).collect())
            }
            Expand::Leaf(l) => Expand::Leaf(make(Some(l.into()))),
        },
        None => Expand::Leaf(make(None)),
    }
}

/// 同 [`expand_wrapped`]，但内层必然存在（`WithType`/`WithTrait` 的盒子非 `Option`）。
fn expand_rebuild<F>(make: F, inner: Ty) -> Expand
where
    F: Fn(Box<Ty>) -> Ty,
{
    match inner.expand() {
        Expand::Many(v) => {
            Expand::Many(v.into_iter().map(|e| make(e.into())).collect())
        }
        Expand::Leaf(l) => Expand::Leaf(make(l.into())),
    }
}

impl Ty {
    /// 展开并列列表类节点：Array 直接拆包，包装类（With*）递归内层并重包。
    ///
    /// [`Expand::Leaf`] = 不可再展开的叶子，节点原样交还（收集为单个 impl）；
    /// [`Expand::Many`] = 展开为多个节点。
    /// 包装类对数组透明透传，使 `<T>[A,B]` 展开为 `<T>A, <T>B`
    /// （泛型声明不重复进单个 impl）；WithAttr/WithPrefix 的透传是防御性的
    /// （数组分发已在 apply 层完成，保持统一透传防未来回归）。
    pub(crate) fn expand(self) -> Expand {
        match self {
            Ty::Array(ty) => Expand::Many(ty.0),
            Ty::WithCode(wc) => {
                let TyWithCode(inner, payload) = wc;
                expand_wrapped(move |i| TyWithCode(i, payload.clone()).into(), inner)
            }
            Ty::WithWhere(ww) => {
                let TyWithWhere(inner, payload) = ww;
                expand_wrapped(move |i| TyWithWhere(i, payload.clone()).into(), inner)
            }
            Ty::WithType(wt) => {
                let TyWithType(params, inner) = wt;
                expand_rebuild(move |e| TyWithType(params.clone(), e).into(), *inner)
            }
            Ty::WithTrait(wt) => {
                let TyWithTrait(t, inner) = wt;
                expand_rebuild(move |e| TyWithTrait(t.clone(), e).into(), *inner)
            }
            Ty::WithAttr(wa) => {
                let TyWithAttr(attr, inner) = wa;
                expand_wrapped(move |i| TyWithAttr(attr.clone(), i).into(), inner)
            }
            Ty::WithPrefix(wp) => {
                let TyWithPrefix(prefix, inner) = wp;
                expand_wrapped(move |i| TyWithPrefix(prefix, i).into(), inner)
            }
            Ty::Group(g) => (*g.0).expand(),
            other => Expand::Leaf(other),
        }
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
                    Box::new(value.into())
                }
            }
            impl From<$struct> for Option<Ty> {
                fn from(value: $struct) -> Self {
                    Some(value.into())
                }
            }
            impl From<$struct> for Option<Box<Ty>> {
                fn from(value: $struct) -> Self {
                    Some(value.into())
                }
            }
        )*
    };
}

impl From<Ty> for Option<Box<Ty>> {
    fn from(ty: Ty) -> Self {
        Some(ty.into())
    }
}

impl_from_for_ty! {
    TyArray => Array,
    TyTuple => Tuple,
    TyGroup => Group,
    TyPrimitiveArray => PrimitiveArray,
    TyPrimitive => Primitive,
    TyGeneric => Generic,
    TyTrait => Trait,
    TyTypeParam => TypeParam,
    TyFn => Fn,
    TyWithPrefix => WithPrefix,
    TyWithAttr => WithAttr,
    TyWithTrait => WithTrait,
    TyWithType => WithType,
    TyWithCode => WithCode,
    TyWithWhere => WithWhere,
    TyNum => Num,
    TyRange => Range,
    TyError => Error,
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

/// 单个展开操作（`^N` / 笛卡尔积 / 范围批量）产物数量上限。
/// 防止 `(T1,..,Tk)^N`、`[A,B]^[C,D]^[E,F]` 等误写指数级膨胀挂起编译
/// （对齐 v0.1 的 1024 上限）。
pub(crate) const MAX_EXPAND: usize = 1024;

/// 统计 `Ty` 树的叶子数（`Array` 逐元素累加，其余计 1）。
/// 用于数组链式分发的产物上限校验。
pub(crate) fn count_leaves(ty: &Ty) -> usize {
    match ty {
        Ty::Array(a) => a.0.iter().map(count_leaves).sum(),
        _ => 1,
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
