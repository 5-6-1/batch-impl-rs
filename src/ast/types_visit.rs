//! `Ty` traversal: parallel expansion ([`Expand`]) and the single child-map
//! home ([`Ty::map_children`]). Split from `types.rs` so node definitions and
//! traversal stay under the per-file budget.

use crate::ast::types::{
    QualifiedHead, Ty, TyArray, TyBoundList, TyFn, TyGeneric, TyGroup, TyKind, TyPack, TyPrefixed,
    TyPrimitiveArray, TyQualified, TyTrait, TyTuple, TyTypeParam, TyWithAttr, TyWithCode,
    TyWithDyn, TyWithFor, TyWithImpl, TyWithPrefix, TyWithTrait, TyWithType, TyWithWhere,
};

pub(crate) enum Expand {
    Leaf(Ty),
    Many(Vec<Ty>),
}

/// Maps a generic parameter list through `f` — the parameter positions of
/// `TyTypeParam`: positional params (name + optional bound) and
/// associated-type bindings (name + value) are all `Ty`-bearing.
fn map_type_param(tp: TyTypeParam, f: &mut impl FnMut(Ty) -> Ty) -> TyTypeParam {
    let params =
        tp.params.into_iter().map(|(n, b)| (f(*n).into(), b.map(|b| f(*b).into()))).collect();
    let bindings =
        tp.bindings.into_iter().map(|(n, v)| (Box::new(f(*n)), Box::new(f(*v)))).collect();
    TyTypeParam { params, bindings }
}

/// Merge two optional fresh declarations (`TyTypeParam::extend` semantics).
pub(crate) fn merge_decls(a: Option<TyTypeParam>, b: Option<TyTypeParam>) -> Option<TyTypeParam> {
    match (a, b) {
        (None, b) => b,
        (a, None) => a,
        (Some(mut a), Some(b)) => {
            a.extend(b);
            Some(a)
        }
    }
}

/// Shared "recurse inner and rewrap" logic for wrapper variants: `make` rebuilds
/// the wrapper from the inner; when `inner` is `None` (bare wrapper), `make(None)`
/// returns it as-is (a leaf). Reused by the WithCode/WithWhere/WithAttr/WithPrefix arms.
pub(super) fn expand_wrapped<F>(make: F, inner: Option<Box<Ty>>) -> Expand
where
    F: Fn(Option<Box<Ty>>) -> Ty,
{
    match inner {
        Some(i) => match i.expand() {
            Expand::Many(v) => Expand::Many(v.into_iter().map(|e| make(e.into())).collect()),
            Expand::Leaf(l) => Expand::Leaf(make(l.into())),
        },
        None => Expand::Leaf(make(None)),
    }
}

/// Like [`expand_wrapped`], but the inner always exists (`WithType`/`WithTrait`
/// boxes are non-`Option`).
pub(super) fn expand_rebuild<F>(make: F, inner: Ty) -> Expand
where
    F: Fn(Box<Ty>) -> Ty,
{
    match inner.expand() {
        Expand::Many(v) => Expand::Many(v.into_iter().map(|e| make(e.into())).collect()),
        Expand::Leaf(l) => Expand::Leaf(make(l.into())),
    }
}

impl Ty {
    /// Maps every child `Ty` node — **including the parameter positions**:
    /// generic argument lists (`T<...>` params + bounds + associated-type
    /// bindings), generic declarations (`<...>` on `WithType`), and trait
    /// argument lists (`WithTrait`) are children too. Rebuilds the node with
    /// its span preserved. Single exhaustive home for the "recurse into
    /// children" pattern — `hoist_type_params`, error collection and future
    /// rebuild-style traversals compose on top of it instead of re-matching
    /// every `TyKind` variant.
    #[allow(clippy::redundant_closure)] // `&mut FnMut` cannot be moved into `.map(f)`
    pub(crate) fn map_children(self, f: &mut impl FnMut(Ty) -> Ty) -> Ty {
        let span = self.span;
        match self.kind {
            TyKind::Array(a) => {
                TyArray(a.0.into_iter().map(|e| f(e)).collect()).to_ty().with_span(span)
            }
            TyKind::Tuple(t) => {
                TyTuple(t.0.into_iter().map(|e| f(e)).collect()).to_ty().with_span(span)
            }
            // Traversal preserves the container; it must not consume splats
            // or packs. Error collection and mass guards need every member.
            TyKind::Pack(p) => {
                TyPack(p.0.into_iter().map(|e| f(e)).collect()).to_ty().with_span(span)
            }
            TyKind::Group(g) => TyGroup(f(*g.0).into()).to_ty().with_span(span),
            TyKind::PrimitiveArray(pa) => {
                TyPrimitiveArray(pa.0.map(|e| f(*e).into()), pa.1).to_ty().with_span(span)
            }
            TyKind::Generic(g) => {
                TyGeneric(f(*g.0).into(), map_type_param(g.1, f)).to_ty().with_span(span)
            }
            TyKind::Trait(t) => TyTrait(t.0, map_type_param(t.1, f)).to_ty().with_span(span),
            TyKind::TypeParam(tp) => map_type_param(tp, f).to_ty().with_span(span),
            TyKind::WithPrefix(wp) => {
                TyWithPrefix(wp.0, wp.1.map(|e| f(*e).into())).to_ty().with_span(span)
            }
            TyKind::Prefixed(p) => TyPrefixed(p.0, f(*p.1).into()).to_ty().with_span(span),
            TyKind::WithDyn(wd) => TyWithDyn(
                Box::new(f(*wd.0)),
                TyBoundList(wd.1.0.into_iter().map(|e| f(e)).collect()),
            )
            .to_ty()
            .with_span(span),
            // The binder is a lifetime list, but it is **part of the node**: the
            // traversals must see it too (an error minted there — `for<u8>` — has
            // to reach the driver's collection, not render into a type position).
            TyKind::WithFor(wf) => {
                TyWithFor(wf.0.into_iter().map(|e| f(e)).collect(), Box::new(f(*wf.1)))
                    .to_ty()
                    .with_span(span)
            }
            TyKind::WithTrait(wt) => {
                TyWithTrait(TyTrait(wt.0.0, map_type_param(wt.0.1, f)), f(*wt.1).into())
                    .to_ty()
                    .with_span(span)
            }
            // A qualified type: the head is a child, the `::`-tail is verbatim
            // tokens (no `Ty` inside it by construction).
            TyKind::Qualified(q) => {
                let head = match q.0 {
                    QualifiedHead::Type(t) => QualifiedHead::Type(f(*t).into()),
                    QualifiedHead::Projection(ty, trait_) => {
                        QualifiedHead::Projection(f(*ty).into(), trait_)
                    }
                };
                TyQualified(head, q.1).to_ty().with_span(span)
            }
            TyKind::WithCode(wc) => {
                TyWithCode(wc.0.map(|e| f(*e).into()), wc.1).to_ty().with_span(span)
            }
            TyKind::WithWhere(ww) => {
                TyWithWhere(ww.0.map(|e| f(*e).into()), ww.1).to_ty().with_span(span)
            }
            TyKind::WithImpl(wi) => {
                TyWithImpl(wi.0.map(|e| f(*e).into()), wi.1).to_ty().with_span(span)
            }
            TyKind::WithType(wt) => {
                TyWithType(map_type_param(wt.0, f), f(*wt.1).into()).to_ty().with_span(span)
            }
            TyKind::WithAttr(wa) => {
                TyWithAttr(wa.0, wa.1.map(|e| f(*e).into())).to_ty().with_span(span)
            }
            TyKind::Fn(fn_) => TyFn(
                fn_.0.map(|params| params.into_iter().map(|p| f(p)).collect()),
                fn_.1.map(|r| f(*r).into()),
                fn_.2,
                fn_.3,
            )
            .to_ty()
            .with_span(span),
            // `+` bound lists recurse element-wise like tuples.
            TyKind::BoundList(b) => {
                TyBoundList(b.0.into_iter().map(|e| f(e)).collect()).to_ty().with_span(span)
            }
            // Keep this exhaustive: adding a structural variant must make
            // its traversal contract an explicit choice, never a silent leaf.
            kind @ (TyKind::Primitive(_)
            | TyKind::Num(_)
            | TyKind::Range(_)
            | TyKind::Fresh(_)
            | TyKind::Lifetime(_)
            | TyKind::Error(_)) => Ty { span, kind },
        }
    }
}
