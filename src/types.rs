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
/// `[...]`
pub(crate) struct TySlice(pub(crate) Box<Ty>);
#[derive(Clone, Debug)]
/// `[...;...]`
pub(crate) struct TyFixedArray(pub(crate) Box<Ty>, pub(crate) TokenStream);
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
/// 关联类型绑定 + `where {...}` 子句。
///
/// `where_clauses` 元素是 `{...}` 内部透传的整段 token 流。
/// codegen 阶段把多条 where_clauses 拼接为 `where P1, P2, ...`
/// 渲染到 impl where 位置。
#[derive(Clone, Debug)]
pub(crate) struct TyTypeParam {
    pub(crate) params: Vec<(TokenStream, Option<Ty>)>,
    pub(crate) bindings: Vec<(TokenStream, TokenStream)>,
    pub(crate) where_clauses: Vec<TokenStream>,
}

impl TyTypeParam {
    /// 构造单个无 bound 参数（`T^U` 中 `U` 变为 `<U>`）
    pub(crate) fn single(arg: &Ty) -> Self {
        TyTypeParam {
            params: vec![(arg.to_token_stream(), None)],
            bindings: vec![],
            where_clauses: vec![],
        }
    }

    /// 追加一个无 bound 参数（`T<A>^B` 中 `B` 追加到 `<A,B>`）
    pub(crate) fn push_arg(&mut self, arg: &Ty) {
        self.params.push((arg.to_token_stream(), None));
    }

    /// 合并另一个参数列表（`T<A>^<B,C>` 中 `<B,C>` 的
    /// params + bindings + where_clauses 合并进来）
    pub(crate) fn extend(&mut self, other: TyTypeParam) {
        self.params.extend(other.params);
        self.bindings.extend(other.bindings);
        self.where_clauses.extend(other.where_clauses);
    }
}
#[derive(Clone, Debug)]
/// `{...}` — 附着在类型上的代码块
pub(crate) struct TyCodeBlock(pub(crate) TokenStream);
#[derive(Clone, Debug)]
/// `T { code }` — 类型 + 代码块
pub(crate) struct TyWithCode(pub(crate) Box<Ty>, pub(crate) TokenStream);
#[derive(Copy, Clone, Debug)]
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

#[derive(Clone, Debug)]
/// prefix `T`
pub(crate) struct TyModified(pub(crate) TyPrefix, pub(crate) Box<Ty>);
#[derive(Clone, Debug)]
/// `fn(...)->T`
pub(crate) struct TyFn(pub(crate) Vec<Ty>, pub(crate) Option<Box<Ty>>);
#[derive(Clone, Debug)]
/// `unsafe T`
pub(crate) struct TyUnsafe(pub(crate) Box<Ty>);
#[derive(Clone, Debug)]
/// `#[...]`
pub(crate) struct TyAttr(pub(crate) TokenStream);
#[derive(Clone, Debug)]
/// `#[...] T`
pub(crate) struct TyWithAttr(pub(crate) TyAttr, pub(crate) Box<Ty>);
#[derive(Copy, Clone, Debug)]
/// `N`
pub(crate) struct TyNum(pub(crate) u8);
#[derive(Copy, Clone, Debug)]
/// `N..M` `N..=M`
pub(crate) struct TyRange {
    pub(crate) start: u8,
    pub(crate) end: u8,
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

/// DSL 解析输出的类型表达式 AST。
///
/// 节点分三类：
/// - **叶子**（Primitive / Num / Range）：不可再展开的原子
/// - **包装**（WithType / WithTrait / WithCode / WithAttr / Unsafe / Modified）：携带元数据，在 codegen 阶段被拆解
/// - **容器**（Array / Tuple / Group / Slice / FixedArray）：可展开为多个叶子的集合
#[derive(Clone, Debug)]
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
    Error(TyError),
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
                    .map(|inner| {
                        Ty::WithCode(TyWithCode(
                            Box::new(inner),
                            wc.1.clone(),
                        ))
                    })
                    .collect()),
                Err(leaf) => {
                    Err(Ty::WithCode(TyWithCode(Box::new(leaf), wc.1)))
                },
            },
            Ty::Group(g) => (*g.0).expand(),
            other => Err(other),
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
