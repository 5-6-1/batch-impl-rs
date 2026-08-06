//! `Ty` traversal: parallel expansion ([`Expand`]) and the single child-map
//! home ([`Ty::map_children`]). Split from `types.rs` so node definitions and
//! traversal stay under the per-file budget.

use crate::ast::types::{
    Ty, TyArray, TyFn, TyGeneric, TyGroup, TyKind, TyPrimitiveArray, TyTuple,
    TyWithAttr, TyWithCode, TyWithPrefix, TyWithTrait, TyWithType, TyWithWhere,
};

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
    /// Maps every child `Ty` node (recursing into lists/optionals), rebuilding
    /// the node with its span preserved. Single exhaustive home for the
    /// "recurse into children" pattern — `hoist_type_params` and future
    /// rebuild-style traversals compose on top of it instead of re-matching
    /// all 18 variants.
    #[allow(clippy::redundant_closure)] // `&mut FnMut` cannot be moved into `.map(f)`
    pub(crate) fn map_children(self, f: &mut impl FnMut(Ty) -> Ty) -> Ty {
        let span = self.span;
        match self.kind {
            TyKind::Array(a) => TyArray(a.0.into_iter().map(|e| f(e)).collect())
                .to_ty()
                .with_span(span),
            TyKind::Tuple(t) => TyTuple(t.0.into_iter().map(|e| f(e)).collect())
                .to_ty()
                .with_span(span),
            TyKind::Group(g) => TyGroup(f(*g.0).into()).to_ty().with_span(span),
            TyKind::PrimitiveArray(pa) => {
                TyPrimitiveArray(pa.0.map(|e| f(*e).into()), pa.1)
                    .to_ty()
                    .with_span(span)
            }
            TyKind::Generic(g) => {
                TyGeneric(f(*g.0).into(), g.1).to_ty().with_span(span)
            }
            TyKind::WithPrefix(wp) => {
                TyWithPrefix(wp.0, wp.1.map(|e| f(*e).into())).to_ty().with_span(span)
            }
            TyKind::WithTrait(wt) => {
                TyWithTrait(wt.0, f(*wt.1).into()).to_ty().with_span(span)
            }
            TyKind::WithCode(wc) => {
                TyWithCode(wc.0.map(|e| f(*e).into()), wc.1).to_ty().with_span(span)
            }
            TyKind::WithWhere(ww) => {
                TyWithWhere(ww.0.map(|e| f(*e).into()), ww.1).to_ty().with_span(span)
            }
            TyKind::WithType(wt) => {
                TyWithType(wt.0, f(*wt.1).into()).to_ty().with_span(span)
            }
            TyKind::WithAttr(wa) => {
                TyWithAttr(wa.0, wa.1.map(|e| f(*e).into())).to_ty().with_span(span)
            }
            TyKind::Fn(fn_) => TyFn(
                fn_.0.map(|params| params.into_iter().map(|p| f(p)).collect()),
                fn_.1.map(|r| f(*r).into()),
                fn_.2,
            )
            .to_ty()
            .with_span(span),
            // No children: keep as-is.
            other => Ty::new(span, other),
        }
    }

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
                    move |i| Ty::new(span, TyWithCode(i, payload.clone()).into()),
                    inner,
                )
            }
            TyKind::WithWhere(ww) => {
                let TyWithWhere(inner, payload) = ww;
                expand_wrapped(
                    move |i| Ty::new(span, TyWithWhere(i, payload.clone()).into()),
                    inner,
                )
            }
            TyKind::WithType(wt) => {
                let TyWithType(params, inner) = wt;
                expand_rebuild(
                    move |e| TyWithType(params.clone(), e).to_ty().with_span(span),
                    *inner,
                )
            }
            TyKind::WithTrait(wt) => {
                let TyWithTrait(t, inner) = wt;
                expand_rebuild(
                    move |e| TyWithTrait(t.clone(), e).to_ty().with_span(span),
                    *inner,
                )
            }
            TyKind::WithAttr(wa) => {
                let TyWithAttr(attr, inner) = wa;
                expand_wrapped(
                    move |i| TyWithAttr(attr.clone(), i).to_ty().with_span(span),
                    inner,
                )
            }
            TyKind::WithPrefix(wp) => {
                let TyWithPrefix(prefix, inner) = wp;
                expand_wrapped(
                    move |i| TyWithPrefix(prefix, i).to_ty().with_span(span),
                    inner,
                )
            }
            TyKind::Group(g) => (*g.0).expand(),
            other => Expand::Leaf(Ty::new(span, other)),
        }
    }
}
