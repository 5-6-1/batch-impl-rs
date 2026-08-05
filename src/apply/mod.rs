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

use quote::quote;

use crate::apply::apply_tuple::map_range;
use crate::ast::*;

/// Build a `Ty::Error` containing `compile_error!` with the given message
pub(crate) fn err_ty(msg: &str) -> Ty {
    TyError(quote! { compile_error!(#msg); }).into()
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
pub(crate) trait Apply:
    Clone + Into<Ty> + Into<Box<Ty>> + Into<Option<Box<Ty>>>
{
    /// Early dispatch for the right operand's "structural context" (default impl, free to all).
    ///
    /// When `o` is Array/Group/WithCode/WithWhere/WithType/Range/Error, it is handled here
    /// (Array dispatch / Group transparency / passthrough / generic hoisting / Range expansion / Error
    /// passthrough); otherwise it delegates to [`Apply::apply_help`] — so `apply_help`'s right operand is
    /// **always a plain type**.
    fn apply(self, o: Ty) -> Ty {
        match o {
            // Array dispatch: apply the left operand to each element of the right array.
            // Array-array chains (`[A,B]^[C,D]^[E,F]`) check the limit by **leaf count** —
            // each intermediate array is small, but leaf count grows exponentially along the `^` chain.
            Ty::Array(arr) => {
                let result: Vec<Ty> =
                    arr.0.into_iter().map(|e| self.clone().apply(e)).collect();
                if let Some(e) = check_expand_limit(
                    "parallel-list chain expansion",
                    result.iter().map(count_leaves).sum(),
                ) {
                    return e;
                }
                TyArray(result).into()
            }
            Ty::Group(g) => self.apply(*g.0),
            Ty::WithCode(wc) => match wc.0 {
                Some(inner) => TyWithCode(self.apply(*inner).into(), wc.1).into(),
                None => TyWithCode(self.into(), wc.1).into(),
            },
            Ty::WithWhere(ww) => match ww.0 {
                Some(inner) => TyWithWhere(self.apply(*inner).into(), ww.1).into(),
                None => TyWithWhere(self.into(), ww.1).into(),
            },
            // When the right operand is `WithType` (e.g. the fresh generic tuple of `()^N`),
            // hoist the generic declaration outward: `T^<A>X` => `<A>(T^X)`,
            // so the type does not leak a generic declaration as `T<<A>X>`.
            Ty::WithType(wt) => TyWithType(wt.0, self.apply(*wt.1).into()).into(),
            Ty::Error(e) => e.into(),
            Ty::Range(TyRange { start, end, inclusive }) => {
                map_range(start, end, inclusive, |n| {
                    self.clone().apply(TyNum(n).into())
                })
            }
            _ => self.apply_help(o),
        }
    }

    /// Left-operand "semantics": each variant implements its own combination rule.
    /// The default [`Apply::apply`] guarantees `o` is a plain type (not a structural context).
    fn apply_help(self, o: Ty) -> Ty;
}

impl Apply for Ty {
    fn apply_help(self, o: Ty) -> Ty {
        match self {
            Ty::WithPrefix(wp) => wp.apply_help(o),
            Ty::Primitive(p) => p.apply_help(o),
            Ty::Generic(g) => g.apply_help(o),
            Ty::Trait(t) => t.apply_help(o),
            Ty::Array(a) => a.apply_help(o),
            Ty::Tuple(t) => t.apply_help(o),
            Ty::Group(g) => g.apply_help(o),
            Ty::Fn(f) => f.apply_help(o),
            Ty::WithAttr(w) => w.apply_help(o),
            Ty::WithTrait(wt) => wt.apply_help(o),
            Ty::WithType(wt) => wt.apply_help(o),
            Ty::WithCode(wc) => wc.apply_help(o),
            Ty::WithWhere(ww) => ww.apply_help(o),
            Ty::TypeParam(t) => t.apply_help(o),
            Ty::Num(n) => n.apply_help(o),
            Ty::Range(r) => r.apply_help(o),
            Ty::PrimitiveArray(pa) => pa.apply_help(o),
            Ty::Error(e) => e.into(),
        }
    }
}

impl Apply for TyWithPrefix {
    /// `&^T` => `&T`; `*const^T` => `*const T`; `self^T` => `T`; `unsafe^T` => `unsafe T`
    /// (unsafe impl marker)
    ///
    /// `&T^U` => `&(T^U)`, `unsafe T^U` => `unsafe (T^U)`: modifiers pass through to the inner type.
    fn apply_help(self, o: Ty) -> Ty {
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
                TyWithPrefix(self.0, inner.into()).into()
            }
            // self^T=>T
            TyPrefix::SelfType => o,
        }
    }
}

impl Apply for TyPrimitive {
    /// `T^U` => `T<U>`; `T^<A,B>` => `T<A,B>`
    fn apply_help(self, o: Ty) -> Ty {
        match o {
            Ty::TypeParam(tp) => TyGeneric(self.into(), tp).into(),
            _ => TyGeneric(self.into(), TyTypeParam::single(&o)).into(),
        }
    }
}

impl Apply for TyGeneric {
    /// `T<A>^B` => `T<A,B>`; `T<A>^<B,C>` => `T<A,B,C>`
    fn apply_help(self, o: Ty) -> Ty {
        let mut tp = self.1;
        match o {
            Ty::TypeParam(rhs) => tp.extend(rhs),
            _ => tp.push_arg(&o),
        }
        TyGeneric(self.0, tp).into()
    }
}

impl Apply for TyTrait {
    /// `Trait<T>^U` => `WithTrait(Trait<T>, U)` (trait generics applied to the target type)
    fn apply_help(self, o: Ty) -> Ty {
        match o {
            Ty::TypeParam(rhs) => {
                let mut tp = self.1;
                tp.extend(rhs);
                TyTrait(self.0, tp).into()
            }
            _ => TyWithTrait(self, o.into()).into(),
        }
    }
}

impl Apply for TyArray {
    /// `[A,B]^C` => `[A^C, B^C]` (right operand is plain; the Cartesian product of `[A,B]^[C,D]`
    /// is dispatched layer-wise by the default `apply` Array branch and flattened via `expand`)
    fn apply_help(self, o: Ty) -> Ty {
        let result = self.0.into_iter().map(|e| e.apply(o.clone())).collect();
        TyArray(result).into()
    }
}
