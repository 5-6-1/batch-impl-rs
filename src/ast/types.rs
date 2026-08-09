use proc_macro2::{Span, TokenStream};
use std::cell::Cell;

#[derive(Clone, Debug)]
/// `[...,]`
pub(crate) struct TyArray(pub(crate) Vec<Ty>);
#[derive(Clone, Debug)]
/// `(...,)`
pub(crate) struct TyTuple(pub(crate) Vec<Ty>);
#[derive(Clone, Debug)]
/// `*[...]` / `*(...)` — splat: flatten a container's elements into the
/// enclosing tuple/array/`^` argument list. Parse-time intermediate: consumed
/// by container collection and apply, so the final AST never contains it.
/// The variant mirrors the source bracket and drives the **left-operand**
/// semantics: `TySplat::Array` distributes `^T` (`*[A^T,B^T]` — set, mirrors
/// `TyArray`), `TySplat::Tuple` appends (`*(A,B,...,T)` — list, mirrors
/// `TyTuple`). Right operands and container collection always flatten
/// regardless of variant.
pub(crate) enum TySplat {
    Tuple(TyTuple),
    Array(TyArray),
}

impl TySplat {
    /// The flattened elements — both variants store the same list shape, so
    /// traversal (expand / render / codegen) reads them through one entry.
    pub(crate) fn elems(&self) -> &[Ty] {
        match self {
            TySplat::Tuple(t) => &t.0,
            TySplat::Array(a) => &a.0,
        }
    }
}
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
///
/// Every element is a `Ty` — non-type tokens (parameter names, `const N`,
/// lifetimes, numeric const args, binding names) ride in a `TyPrimitive`
/// wrapper, so render / traversal / apply treat params uniformly as
/// structured types. The same shape serves both **declarations**
/// (`<T: Bound>`, no base) and **arguments** (`T<A>`, base present) — the
/// distinction lives in the render function used
/// (`params_to_tokens` vs `params_to_tokens_no_base`).
#[derive(Clone, Debug)]
pub(crate) struct TyTypeParam {
    pub(crate) params: Vec<(Box<Ty>, Option<Ty>)>,
    pub(crate) bindings: Vec<(Box<Ty>, Box<Ty>)>,
}

impl TyTypeParam {
    /// Constructs a single unbound param (`U` in `T^U` becomes `<U>`)
    pub(crate) fn single(arg: &Ty) -> Self {
        TyTypeParam {
            params: vec![(Box::new(arg.clone()), None)],
            bindings: vec![],
        }
    }

    /// Appends an unbound param (`B` in `T<A>^B` appends to `<A,B>`)
    pub(crate) fn push_arg(&mut self, arg: &Ty) {
        self.params.push((Box::new(arg.clone()), None));
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
    /// Chainable constructor: builds a node from a subtype's `into()` and
    /// overrides its span (`TyArray(x).into().with_span(span)`).
    pub(crate) fn with_span(mut self, span: Span) -> Self {
        self.span = span;
        self
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
    Splat(TySplat),
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
    static GROUP_COUNTER: Cell<usize> = 0.into();
}

/// Resets the fresh-generator group counter (per spec / `batch_trait!`
/// segment, so group ids are DSL-local; the codegen sweeper renumbers every
/// impl's fresh params to `_Param_0..N_BatchGen_` afterwards).
pub(crate) fn reset_fresh_counter() {
    GROUP_COUNTER.set(0);
}

/// Takes the next fresh-generator group id.
pub(crate) fn take_group() -> usize {
    GROUP_COUNTER.with(|c| {
        let g = c.get();
        c.set(g + 1);
        g
    })
}
