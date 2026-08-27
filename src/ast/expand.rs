// Parallel-list expansion ([Expand]): the driver-stage flattening of
// Array / Splat / wrapper nodes into leaf Tys — the counterpart of the
// traversal concern in types_visit.rs. Split by concern so both stay under
// the per-file budget.

use super::types_visit::{expand_rebuild, expand_wrapped, splat_expand};
use crate::apply::expand_limit_err;
use crate::ast::Ty;
use crate::ast::types::*;
use crate::ast::{Expand, MAX_EXPAND};
use crate::util::cartesian;

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
            // Top-level splat: consume (flatten containers/generators) and
            // distribute like a list. Fresh declarations from flattened
            // generators wrap each distributed element.
            TyKind::Splat(s) => {
                let (elems, decl) = splat_expand(s.to_ty().with_span(span));
                Expand::Many(
                    elems
                        .into_iter()
                        .map(|e| match &decl {
                            Some(d) => TyWithType(d.clone(), e.into()).to_ty().with_span(span),
                            None => e,
                        })
                        .collect(),
                )
            }
            TyKind::Tuple(t) => {
                // List distribution: an array element (a dispatch list) makes
                // the tuple expand by Cartesian product — `(X, [A, B])` →
                // `(X, A)`, `(X, B)`. Combos that still contain arrays
                // re-expand through the driver work queue (recursive
                // distribution, which also covers pow_cartesian outputs
                // nested in outer tuples). Pure tuples stay a Leaf.
                if t.0.iter().any(|e| matches!(e.kind, TyKind::Array(_))) {
                    let dims =
                        t.0.iter()
                            .map(|e| match &e.kind {
                                TyKind::Array(a) => a.0.clone(),
                                _ => vec![e.clone()],
                            })
                            .collect::<Vec<_>>();
                    let combos = match cartesian(&dims, MAX_EXPAND) {
                        Ok(c) => c,
                        Err(size) => {
                            return expand_limit_err("tuple list distribution", size).expand();
                        }
                    };
                    Expand::Many(
                        combos
                            .into_iter()
                            .map(|combo| TyTuple(combo).to_ty().with_span(span))
                            .collect(),
                    )
                } else {
                    Expand::Leaf(t.to_ty().with_span(span))
                }
            }
            TyKind::WithCode(wc) => {
                let TyWithCode(inner, payload) = wc;
                expand_wrapped(
                    move |i| TyWithCode(i, payload.clone()).to_ty().with_span(span),
                    inner,
                )
            }
            TyKind::WithWhere(ww) => {
                let TyWithWhere(inner, payload) = ww;
                expand_wrapped(
                    move |i| TyWithWhere(i, payload.clone()).to_ty().with_span(span),
                    inner,
                )
            }
            TyKind::WithImpl(wi) => {
                let TyWithImpl(inner, payload) = wi;
                expand_wrapped(
                    move |i| TyWithImpl(i, payload.clone()).to_ty().with_span(span),
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
                expand_rebuild(move |e| TyWithTrait(t.clone(), e).to_ty().with_span(span), *inner)
            }
            TyKind::WithAttr(wa) => {
                let TyWithAttr(attr, inner) = wa;
                expand_wrapped(move |i| TyWithAttr(attr.clone(), i).to_ty().with_span(span), inner)
            }
            TyKind::WithPrefix(wp) => {
                let TyWithPrefix(prefix, inner) = wp;
                expand_wrapped(move |i| TyWithPrefix(prefix, i).to_ty().with_span(span), inner)
            }
            TyKind::WithDyn(wd) => {
                let TyWithDyn(inner, bounds) = wd;
                expand_rebuild(
                    move |i| TyWithDyn(i, bounds.clone()).to_ty().with_span(span),
                    *inner,
                )
            }
            TyKind::WithFor(wf) => {
                let TyWithFor(binder, inner) = wf;
                expand_rebuild(
                    move |i| TyWithFor(binder.clone(), i).to_ty().with_span(span),
                    *inner,
                )
            }
            TyKind::Group(g) => (*g.0).expand(),
            TyKind::Generic(g) => {
                // Array args distribute like a list — `T<[A,B]>` → `[T<A>, T<B>]`
                // (Cartesian across multiple arrays). This is the single
                // authority for array-arg distribution: literal `[A,B]`, the
                // `[u8,...]` from a `@u*` constant, and the `TyArray` produced
                // by splat powers (`*(*@u*).2` → `[*(u8,u8), ...]`) all reach
                // params as a `TyArray` and distribute here.
                if g.1.params.iter().any(|(n, _)| matches!(n.kind, TyKind::Array(_))) {
                    let dims =
                        g.1.params
                            .iter()
                            .map(|(name, bound)| match &name.kind {
                                TyKind::Array(a) => {
                                    a.0.iter().map(|e| (e.clone().into(), bound.clone())).collect()
                                }
                                _ => vec![(name.clone(), bound.clone())],
                            })
                            .collect::<Vec<_>>();
                    let combos = match cartesian(&dims, MAX_EXPAND) {
                        Ok(c) => c,
                        Err(size) => {
                            return expand_limit_err("generic array distribution", size).expand();
                        }
                    };
                    Expand::Many(
                        combos
                            .into_iter()
                            .map(|params| {
                                TyGeneric(
                                    g.0.clone(),
                                    TyTypeParam { params, bindings: g.1.bindings.clone() },
                                )
                                .to_ty()
                                .with_span(span)
                            })
                            .collect(),
                    )
                } else {
                    Expand::Leaf(Ty { span, kind: TyKind::Generic(g) })
                }
            }
            other => Expand::Leaf(Ty { span, kind: other }),
        }
    }
}
