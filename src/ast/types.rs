use proc_macro2::{Span, TokenStream};
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
/// `[]` (seed) / `[T]` (slice) / `[T; N]` (fixed-length array) — `None` element
/// means empty `[]`, `None` length means slice
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
/// `<T: Clone, U, Item=V>` generic parameter list: positional params (with optional
/// bounds) + associated type bindings.
#[derive(Clone, Debug)]
pub(crate) struct TyTypeParam {
    pub(crate) params: Vec<(TokenStream, Option<Ty>)>,
    pub(crate) bindings: Vec<(TokenStream, TokenStream)>,
}

impl TyTypeParam {
    /// Constructs a single unbound param (`U` in `T^U` becomes `<U>`)
    pub(crate) fn single(arg: &Ty) -> Self {
        TyTypeParam {
            params: vec![(arg.to_token_stream(), None)],
            bindings: vec![],
        }
    }

    /// Appends an unbound param (`B` in `T<A>^B` appends to `<A,B>`)
    pub(crate) fn push_arg(&mut self, arg: &Ty) {
        self.params.push((arg.to_token_stream(), None));
    }

    /// Merges another param list (the `<B,C>` in `T<A>^<B,C>` has its
    /// params + bindings merged in)
    pub(crate) fn extend(&mut self, other: TyTypeParam) {
        self.params.extend(other.params);
        self.bindings.extend(other.bindings);
    }
}
#[derive(Clone, Debug)]
/// `{...}` — a code block attached to a type
pub(crate) struct TyCodeBlock(pub(crate) TokenStream);
#[derive(Clone, Debug)]
/// `{...}` (bare) or `T { code }` — inner `None` means a bare code block. In codegen
/// a bare block is emitted verbatim as a top-level item via this path (for the
/// degenerate form of an instruction alone as the whole spec: the `{name!{...}}` block
/// from `#name(args){body}` is a normal impl body when attached to a type, and a
/// standalone top-level item here).
pub(crate) struct TyWithCode(pub(crate) Option<Box<Ty>>, pub(crate) TyCodeBlock);
#[derive(Copy, Clone, Debug)]
/// `& &mut *const *mut self unsafe` — type prefix modifiers
pub(crate) enum TyPrefix {
    Ref,
    RefMut,
    PtrConst,
    PtrMut,
    SelfType,
    Unsafe,
}

#[derive(Clone, Debug)]
/// Bare prefix (`&`/`unsafe` etc.) or `prefix T` — inner `None` means a bare prefix
pub(crate) struct TyWithPrefix(pub(crate) TyPrefix, pub(crate) Option<Box<Ty>>);
#[derive(Clone, Debug)]
/// Bare `fn` / `fn(...)` / `fn(...)->T` — params `None` means not filled yet; the
/// third field `is_unsafe` marks `unsafe fn(...)` types (`unsafe` qualifies the fn
/// type itself, as opposed to `unsafe^T` marking an unsafe impl)
pub(crate) struct TyFn(
    pub(crate) Option<Vec<Ty>>,
    pub(crate) Option<Box<Ty>>,
    pub(crate) bool,
);
#[derive(Clone, Debug)]
/// `#[...]` — the attribute itself
pub(crate) struct TyAttr(pub(crate) TokenStream);
#[derive(Clone, Debug)]
/// `#[...]` (bare) or `#[...] T` — inner `None` means a bare attribute
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
/// Compile-time error signal — produced on invalid DSL semantics, finally emits `compile_error!`
pub(crate) struct TyError(pub(crate) TokenStream);

#[derive(Clone, Debug)]
pub(crate) struct TyWhere(pub(crate) TokenStream);

#[derive(Clone, Debug)]
/// Bare `where{...}` or `T where{...}` — inner `None` means a bare where suffix
pub(crate) struct TyWithWhere(pub(crate) Option<Box<Ty>>, pub(crate) TyWhere);

#[derive(Clone, Debug)]
pub(crate) struct Ty {
    /// Source span: the user-written token(s) this node came from;
    /// `Span::call_site()` for macro-generated nodes (fresh generics,
    /// `@`-expansion products, directive output).
    pub(crate) span: Span,
    pub(crate) kind: TyKind,
}

impl Ty {
    pub(crate) fn new(span: Span, kind: TyKind) -> Self {
        Ty { span, kind }
    }
}

/// The type expression AST produced by DSL parsing.
///
/// Nodes fall into three categories:
/// - **Leaf** (Primitive / Num / Range): an atomic that cannot expand further
/// - **Wrapper** (WithType / WithTrait / WithPrefix / WithCode / WithWhere /
///   WithAttr / Fn): carries metadata, dismantled in the codegen phase
/// - **Container** (Array / Tuple / Group / PrimitiveArray): expands into leaves
///
/// Prefix/suffix wrappers (WithPrefix / WithCode / WithAttr / WithWhere) use
/// `Option<Box<Ty>>` for the bare "no type attached yet" state, avoiding half-built
/// variants; `Fn` tracks its params as `Option<Vec<Ty>>` (bare `fn` = `None`) with the
/// return type as `Option<Box<Ty>>`.
#[derive(Clone, Debug)]
pub(crate) enum TyKind {
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
/// Result of [`Ty::expand`]: `Leaf` = non-expandable leaf; `Many` = expands into multiple nodes.
pub(crate) enum Expand {
    Leaf(Ty),
    Many(Vec<Ty>),
}

/// Shared "recurse inner and rewrap" logic for wrapper variants: `make` rebuilds
/// the wrapper from the inner; when `inner` is `None` (bare wrapper), `make(None)`
/// returns it as-is (a leaf). Reused by the WithCode/WithWhere/WithAttr/WithPrefix arms.
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

/// Like [`expand_wrapped`], but the inner always exists (`WithType`/`WithTrait`
/// boxes are non-`Option`).
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
    /// Expands parallel-list nodes: `Array` unwraps directly, wrappers (With*) recurse.
    ///
    /// [`Expand::Leaf`] = non-expandable leaf returned as-is (collected as one impl);
    /// [`Expand::Many`] = expands into multiple nodes. Wrappers pass arrays through
    /// transparently: `<T>[A,B]` becomes `<T>A, <T>B` (generic declarations are not
    /// repeated into a single impl); WithAttr/WithPrefix passthrough is defensive
    /// (array dispatch already happens in apply; uniform passthrough prevents regressions).
    pub(crate) fn expand(self) -> Expand {
        let Ty { span, kind } = self;
        match kind {
            TyKind::Array(ty) => Expand::Many(ty.0),
            TyKind::WithCode(wc) => {
                let TyWithCode(inner, payload) = wc;
                expand_wrapped(
                    move |i| {
                        Ty::new(
                            span,
                            TyKind::WithCode(TyWithCode(i, payload.clone())),
                        )
                    },
                    inner,
                )
            }
            TyKind::WithWhere(ww) => {
                let TyWithWhere(inner, payload) = ww;
                expand_wrapped(
                    move |i| {
                        Ty::new(
                            span,
                            TyKind::WithWhere(TyWithWhere(i, payload.clone())),
                        )
                    },
                    inner,
                )
            }
            TyKind::WithType(wt) => {
                let TyWithType(params, inner) = wt;
                expand_rebuild(
                    move |e| {
                        Ty::new(span, TyKind::WithType(TyWithType(params.clone(), e)))
                    },
                    *inner,
                )
            }
            TyKind::WithTrait(wt) => {
                let TyWithTrait(t, inner) = wt;
                expand_rebuild(
                    move |e| {
                        Ty::new(span, TyKind::WithTrait(TyWithTrait(t.clone(), e)))
                    },
                    *inner,
                )
            }
            TyKind::WithAttr(wa) => {
                let TyWithAttr(attr, inner) = wa;
                expand_wrapped(
                    move |i| {
                        Ty::new(span, TyKind::WithAttr(TyWithAttr(attr.clone(), i)))
                    },
                    inner,
                )
            }
            TyKind::WithPrefix(wp) => {
                let TyWithPrefix(prefix, inner) = wp;
                expand_wrapped(
                    move |i| {
                        Ty::new(span, TyKind::WithPrefix(TyWithPrefix(prefix, i)))
                    },
                    inner,
                )
            }
            TyKind::Group(g) => (*g.0).expand(),
            other => Expand::Leaf(Ty::new(span, other)),
        }
    }
}

macro_rules! impl_from_for_ty {
    ($($struct:ident => $variant:ident),* $(,)?) => {
        $(
            impl From<$struct> for Ty {
                fn from(value: $struct) -> Self {
                    Ty::new(Span::call_site(), TyKind::$variant(value))
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

/// Operator precedence levels (low→high: `;` < `,` < `-` < `^`; `Prim` = atomic, no operator).
///
/// Each level defines "stop characters": when scanning at that level, `parse_operand`
/// truncates at them, then hands the truncated slice to higher-precedence recursion.
#[derive(Copy, Clone)]
pub(crate) enum Op {
    Semi,
    Comma,
    Dash,
    Caret,
    Prim,
}

impl Op {
    /// The next-higher precedence level
    pub(crate) fn next(self) -> Option<Op> {
        match self {
            Op::Semi => Some(Op::Comma),
            Op::Comma => Some(Op::Dash),
            Op::Dash => Some(Op::Caret),
            Op::Caret => Some(Op::Prim),
            Op::Prim => None,
        }
    }

    /// Characters at which the operand is truncated at this level
    pub(crate) fn stop_chars(self) -> &'static [char] {
        match self {
            // Semi also stops at `,`: it cuts item/paragraph boundaries; the caller distinguishes them
            Op::Semi => &[',', ';'],
            Op::Comma => &[','],
            Op::Dash => &['-', ','],
            Op::Caret => &['^', '-', ','],
            Op::Prim => &[],
        }
    }
}

/// Upper bound on the products of a single expansion (`^N` / cartesian / range batch).
/// Prevents exponential blowups like `(T1,..,Tk)^N`, `[A,B]^[C,D]^[E,F]` from hanging
/// compilation (aligned with the v0.1 cap of 1024).
pub(crate) const MAX_EXPAND: usize = 1024;

/// Counts leaves in a `Ty` tree (`Array` sums per element, everything else counts 1).
/// Used to validate the product cap of chained array dispatch.
pub(crate) fn count_leaves(ty: &Ty) -> usize {
    match &ty.kind {
        TyKind::Array(a) => a.0.iter().map(count_leaves).sum(),
        _ => 1,
    }
}

thread_local! {
    static FRESH_COUNTER: Cell<usize> = 0.into();
}

/// Resets the fresh param counter (called once per macro entry so generated
/// generic names do not collide across macros)
pub(crate) fn reset_fresh_counter() {
    FRESH_COUNTER.set(0);
}

/// Generates a fresh generic param name that never collides with user code
/// (`_Param_0_BatchGen_`, `_Param_1_BatchGen_`, ...)
pub(crate) fn fresh_param() -> TokenStream {
    FRESH_COUNTER.with(|c| {
        let n = c.get();
        c.set(n + 1);
        let name = format!("_Param_{}_BatchGen_", n);
        let ident = Ident::new(&name, proc_macro2::Span::call_site());
        quote!(#ident)
    })
}
