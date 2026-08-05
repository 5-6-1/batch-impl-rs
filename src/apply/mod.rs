//! Apply layer: the `Apply` trait and operator semantics for each `Ty` variant.

pub(crate) mod apply_tuple;

// The [`Apply`] trait defines the binary operation `A.apply(B)`: `^` (right-assoc) / `-` (left-assoc).
// Each `Ty` variant implements [`Apply::apply_help`] with its combination semantics — containers
// append args, references wrap, lists take a Cartesian product, tuples expand by length (`()^N`,
// `(<Bound>)^N`), associated parameters are generated, etc. The **early dispatch of the right
// operand's "structural context"** (Array dispatch / Group transparency / WithCode & WithWhere
// passthrough / WithType generic hoisting / Range expansion / Error passthrough) lives in the
// default [`Apply::apply`] — every `Apply` impl gets it for free, no repetition.
//
// Right-operand structural dispatch is part of the trait contract.

use quote::{quote, quote_spanned};

use crate::apply::apply_tuple::map_range;
use crate::ast::*;
use proc_macro2::Span;

/// Build a `Ty::Error` containing `compile_error!` with the given message
/// (call-site span).
pub(crate) fn err_ty(msg: &str) -> Ty {
    Ty::new(
        proc_macro2::Span::call_site(),
        TyKind::Error(TyError(quote! { compile_error!(#msg); })),
    )
}

/// `err_ty` with an explicit span: the error renders at `span` (the offending
/// token / `Ty::span` / the apply `span` parameter in hand at the error site).
pub(crate) fn err_ty_at(msg: &str, span: Span) -> Ty {
    let ts = quote_spanned!(span => compile_error!(#msg););
    Ty::new(span, TyKind::Error(TyError(ts)))
}

/// Expansion-count check: returns a `compile_error!` signal when `len` exceeds [`MAX_EXPAND`].
/// Used where expansion can blow up exponentially: `^N` / Cartesian products / ranges
pub(crate) fn check_expand_limit(what: &str, len: usize) -> Option<Ty> {
    (len > MAX_EXPAND).then(|| {
        err_ty(&format!(
            "batch-impl: `{}` expands to {} items (limit {}); likely exponential/range/Cartesian typo",
            what, len, MAX_EXPAND
        ))
    })
}

/// Binary operation on type expressions: in `A^B` / `A-B`, `A.apply(B)` combines into a `Ty`.
///
/// Needs `Clone` (default Array dispatch / Range expansion reuse left operand `self`) and
/// `Into<Ty> + Into<Box<Ty>> + Into<Option<Box<Ty>>>` (to put the left operand back into a type /
/// target type — used for bare code blocks and bare where as the right operand, and when variants
/// like `TyPrimitive` convert `self` into a generic base).
/// Binary operation on type expressions: in `A^B` / `A-B`, `A.apply(B)` combines into a `Ty`.
///
/// Needs `Clone` (default Array dispatch / Range expansion reuse left operand).
/// The span of the left operand is threaded through every method (`apply` /
/// `apply_help`) so combinator output keeps the left operand's source position.
pub(crate) trait Apply: Clone + Into<Ty> {
    /// Left-operand "semantics": each variant implements its own combination rule.
    /// Called by [`TyKind::apply`] only after right-operand structural dispatch —
    /// so `o` is **always a plain type** (not an Array/Group/With*/Range/Error context).
    /// `span` is the left operand's span; combinator output is built via
    /// [`Ty::new`]`(span, ...)` so it keeps the left operand's source position.
    fn apply_help(self, o: Ty, span: Span) -> Ty;
}

/// `Ty::apply`: takes the node's own span, delegates to the kind's logic, and
/// reconstructs with that span — the single place where `span` flows into
/// combinator output.
impl Ty {
    pub(crate) fn apply(self, o: Ty) -> Ty {
        let Ty { span, kind } = self;
        kind.apply(o, span)
    }
}

impl TyKind {
    /// Right-operand structural dispatch (Array / Group / WithCode / WithWhere /
    /// WithType / Range / Error are handled here; anything else falls through to
    /// [`TyKind::apply_help`]). `span` is the left operand's span, applied to
    /// every constructed node.
    pub(crate) fn apply(self, o: Ty, span: Span) -> Ty {
        match o.kind {
            // Array dispatch: apply the left operand to each element of the right array.
            // Array-array chains (`[A,B]^[C,D]^[E,F]`) check the limit by **leaf count** —
            // each intermediate array is small, but leaf count grows exponentially along the `^` chain.
            TyKind::Array(arr) => {
                let result: Vec<Ty> = arr
                    .0
                    .into_iter()
                    .map(|e| Ty::new(span, self.clone()).apply(e))
                    .collect();
                if let Some(e) = check_expand_limit(
                    "parallel-list chain expansion",
                    result.iter().map(count_leaves).sum(),
                ) {
                    return e;
                }
                Ty::new(span, TyKind::Array(TyArray(result)))
            }
            TyKind::Group(g) => Ty::new(span, self).apply(*g.0),
            TyKind::WithCode(wc) => match wc.0 {
                Some(inner) => Ty::new(
                    span,
                    TyKind::WithCode(TyWithCode(
                        Ty::new(span, self.clone()).apply(*inner).into(),
                        wc.1,
                    )),
                ),
                None => Ty::new(
                    span,
                    TyKind::WithCode(TyWithCode(
                        Some(Box::new(Ty::new(span, self))),
                        wc.1,
                    )),
                ),
            },
            TyKind::WithWhere(ww) => match ww.0 {
                Some(inner) => Ty::new(
                    span,
                    TyKind::WithWhere(TyWithWhere(
                        Ty::new(span, self.clone()).apply(*inner).into(),
                        ww.1,
                    )),
                ),
                None => Ty::new(
                    span,
                    TyKind::WithWhere(TyWithWhere(
                        Some(Box::new(Ty::new(span, self))),
                        ww.1,
                    )),
                ),
            },
            // When the right operand is `WithType` (e.g. the fresh generic tuple of `()^N`),
            // hoist the generic declaration outward: `T^<A>X` => `<A>(T^X)`,
            // so the type does not leak a generic declaration as `T<<A>X>`.
            // But when self is itself a generic declaration (`<'a>^<T>X` — the
            // `<'a> <T> X` consecutive-declaration form), hoisting would reorder
            // lifetimes after type params (E0xxx); keep declaration order via
            // `WithType(self, o)` so `<'a, T>` stays lifetimes-first.
            TyKind::WithType(wt) if matches!(self, TyKind::TypeParam(_)) => {
                self.apply_help(Ty::new(o.span, TyKind::WithType(wt)), span)
            }
            TyKind::WithType(wt) => Ty::new(
                span,
                TyKind::WithType(TyWithType(
                    wt.0,
                    Ty::new(span, self.clone()).apply(*wt.1).into(),
                )),
            ),
            TyKind::Error(e) => Ty::new(span, TyKind::Error(e)),
            TyKind::Range(TyRange { start, end, inclusive }) => {
                map_range(start, end, inclusive, span, |n| {
                    Ty::new(span, self.clone())
                        .apply(Ty::new(span, TyKind::Num(TyNum(n))))
                })
            }
            other => self.apply_help(Ty::new(o.span, other), span),
        }
    }

    /// Forwards to the concrete subtype's [`Apply::apply_help`] (each variant
    /// implements its own combination rule).
    fn apply_help(self, o: Ty, span: Span) -> Ty {
        match self {
            TyKind::WithPrefix(wp) => wp.apply_help(o, span),
            TyKind::Primitive(p) => p.apply_help(o, span),
            TyKind::Generic(g) => g.apply_help(o, span),
            TyKind::Trait(t) => t.apply_help(o, span),
            TyKind::Array(a) => a.apply_help(o, span),
            TyKind::Tuple(t) => t.apply_help(o, span),
            TyKind::Group(g) => g.apply_help(o, span),
            TyKind::Fn(f) => f.apply_help(o, span),
            TyKind::WithAttr(w) => w.apply_help(o, span),
            TyKind::WithTrait(wt) => wt.apply_help(o, span),
            TyKind::WithType(wt) => wt.apply_help(o, span),
            TyKind::WithCode(wc) => wc.apply_help(o, span),
            TyKind::WithWhere(ww) => ww.apply_help(o, span),
            TyKind::TypeParam(t) => t.apply_help(o, span),
            TyKind::Num(n) => n.apply_help(o, span),
            TyKind::Range(r) => r.apply_help(o, span),
            TyKind::PrimitiveArray(pa) => pa.apply_help(o, span),
            TyKind::Error(e) => Ty::new(span, TyKind::Error(e)),
        }
    }
}

impl Apply for TyWithPrefix {
    /// `&^T` => `&T`; `*const^T` => `*const T`; `self^T` => `T`; `unsafe^T` => `unsafe T`
    /// (unsafe impl marker)
    ///
    /// `&T^U` => `&(T^U)`, `unsafe T^U` => `unsafe (T^U)`: modifiers pass through to the inner type.
    fn apply_help(self, o: Ty, span: Span) -> Ty {
        match self.0 {
            // &^T=>&T / unsafe^T=>unsafe T
            TyPrefix::Ref
            | TyPrefix::RefMut
            | TyPrefix::PtrConst
            | TyPrefix::PtrMut
            | TyPrefix::Unsafe => {
                let inner = match self.1 {
                    Some(t) => t.apply(o),
                    None => o,
                };
                Ty::new(span, TyKind::WithPrefix(TyWithPrefix(self.0, inner.into())))
            }
            // self^T=>T
            TyPrefix::SelfType => o,
        }
    }
}

impl Apply for TyPrimitive {
    /// `T^U` => `T<U>`; `T^<A,B>` => `T<A,B>`
    fn apply_help(self, o: Ty, span: Span) -> Ty {
        let o_span = o.span;
        match o.kind {
            TyKind::TypeParam(tp) => {
                Ty::new(span, TyKind::Generic(TyGeneric(self.into(), tp)))
            }
            _ => Ty::new(
                span,
                TyKind::Generic(TyGeneric(
                    self.into(),
                    TyTypeParam::single(&Ty::new(o_span, o.kind)),
                )),
            ),
        }
    }
}

impl Apply for TyGeneric {
    /// `T<A>^B` => `T<A,B>`; `T<A>^<B,C>` => `T<A,B,C>`
    fn apply_help(self, o: Ty, span: Span) -> Ty {
        let mut tp = self.1;
        let o_span = o.span;
        match o.kind {
            TyKind::TypeParam(rhs) => tp.extend(rhs),
            _ => tp.push_arg(&Ty::new(o_span, o.kind)),
        }
        Ty::new(span, TyKind::Generic(TyGeneric(self.0, tp)))
    }
}

impl Apply for TyTrait {
    /// `Trait<T>^U` => `WithTrait(Trait<T>, U)` (trait generics applied to the target type)
    fn apply_help(self, o: Ty, span: Span) -> Ty {
        let o_span = o.span;
        match o.kind {
            TyKind::TypeParam(rhs) => {
                let mut tp = self.1;
                tp.extend(rhs);
                Ty::new(span, TyKind::Trait(TyTrait(self.0, tp)))
            }
            _ => Ty::new(
                span,
                TyKind::WithTrait(TyWithTrait(self, Ty::new(o_span, o.kind).into())),
            ),
        }
    }
}

impl Apply for TyArray {
    /// `[A,B]^C` => `[A^C, B^C]` (right operand is plain; the Cartesian product of `[A,B]^[C,D]`
    /// is dispatched layer-wise by the default `apply` Array branch and flattened via `expand`)
    fn apply_help(self, o: Ty, span: Span) -> Ty {
        let result = self.0.into_iter().map(|e| e.apply(o.clone())).collect();
        Ty::new(span, TyKind::Array(TyArray(result)))
    }
}
