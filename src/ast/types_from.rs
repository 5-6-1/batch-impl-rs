//! Subtype conversion impls: the uniform [impl_from_for_ty!] macro (subtype
//! → `TyKind` / `Ty` / `Box<Ty>` + the `to_ty` chainable entry point). Split from
//! types.rs so node definitions and conversions stay under the per-file budget.

use proc_macro2::Span;

use super::types::*;

macro_rules! impl_from_for_ty {
    ($($struct:ident => $variant:ident),* $(,)?) => {
        $(
            // Subtype → TyKind (pure structural conversion, no span semantics).
            // Enables `TyArray(x).into()` → `TyArray(x).into()`.
            impl From<$struct> for TyKind {
                fn from(value: $struct) -> Self {
                    TyKind::$variant(value)
                }
            }
            // Subtype → full Ty (call_site span). `to_ty()` builds on this.
            impl From<$struct> for Ty {
                fn from(value: $struct) -> Self {
                    Ty { span: Span::call_site(), kind: TyKind::$variant(value) }
                }
            }
            // Chainable entry point: explicit return type resolves `self.into()`
            // (`.into()` alone cannot be back-inferred by `with_span` — E0282).
            // Takes ownership by design (moves into Ty), so clippy's by-ref
            // `to_*` convention hint does not apply; unexercised subtypes keep
            // the method as part of the uniform constructor surface.
            #[allow(clippy::wrong_self_convention, dead_code)]
            impl $struct {
                pub(crate) fn to_ty(self) -> Ty {
                    self.into()
                }
            }
            // Kept: the `Expand` traversers build `Some(e.into())` expecting Box<Ty>.
            // `value.into()` alone would recurse into this very impl (target
            // `Box<Ty>` matches itself); inside `Box::new` the target is the
            // argument — inferred as `Ty` via `From<$t> for Ty` (non-recursive).
            // `Box::new` here is the definition site (the only way to build a
            // Box), not a usage-site wrapper.
            impl From<$struct> for Box<Ty> {
                fn from(value: $struct) -> Self {
                    Box::new(value.into())
                }
            }
        )*
    };
}

impl_from_for_ty! {
    TyArray => Array,
    TyTuple => Tuple,
    TySplat => Splat,
    TyGroup => Group,
    TyPrimitiveArray => PrimitiveArray,
    TyPrimitive => Primitive,
    TyGeneric => Generic,
    TyTrait => Trait,
    TyTypeParam => TypeParam,
    TyFn => Fn,
    TyWithPrefix => WithPrefix,
    TyWithDyn => WithDyn,
    TyWithFor => WithFor,
    TyWithAttr => WithAttr,
    TyWithTrait => WithTrait,
    TyWithType => WithType,
    TyWithCode => WithCode,
    TyWithWhere => WithWhere,
    TyWithImpl => WithImpl,
    TyNum => Num,
    TyRange => Range,
    TyFresh => Fresh,
    TyLifetime => Lifetime,
    TyBoundList => BoundList,
    TyError => Error,
}

impl From<Ty> for Option<Box<Ty>> {
    fn from(ty: Ty) -> Self {
        Some(ty.into())
    }
}
