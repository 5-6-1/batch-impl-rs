//! Splat (`*` prefix) left-operand semantics. Split from mod.rs so the
//! Apply-layer entry file stays under the line budget.

use proc_macro2::Span;

use crate::apply::Apply;
use crate::ast::*;

impl Apply for TySplat {
    /// Left-operand splat — fully delegates to the mirrored container, then
    /// re-wraps the result as a splat (the `*` flattening survives until
    /// consumption):
    /// - `TySplat::Array` → `TyArray` distribution (`*[A,B]^T` = `*[A^T,B^T]`,
    ///   re-wrapped so right-splat chains can flatten into a container)
    /// - `TySplat::Tuple` → `TyTuple` append (`*(A,B)^T` = `*(A,B,...,T)`,
    ///   re-wrapped); `^N` pow re-wraps each Cartesian combo into a splat
    ///   (`*(A,B)^2` = `[*(A,A),*(A,B),*(B,A),*(B,B)]` — param-position
    ///   lists a right-splat chain flattens into a container);
    ///   `*()^N` re-wraps its fresh tuple into the splat
    ///   (`T^*()^2` = `<A,B>T<A,B>`).
    fn apply_help(self, o: Ty, span: Span) -> Ty {
        match self {
            // `*[A,B]^T` — distribution: every element gets `^T`, then the
            // splat is kept so a right-splat chain can flatten the elements
            // into a container (`Pair^*[A,B]^T` = `Pair<A^T, B^T>`).
            TySplat::Array(a) => {
                let applied = match a.apply(o, span).kind {
                    TyKind::Array(na) => na,
                    other => return Ty { span, kind: other },
                };
                TySplat::Array(applied).to_ty().with_span(span)
            }
            // `*(...)` — appending (`^T`) keeps the splat; `^N` pow
            // re-wraps Cartesian combos into splats; `*()^N` (empty splat)
            // re-wraps its fresh tuple into the splat so a carrier appends
            // the params into `T` (`T^*()^2` = `<A,B>T<A,B>`; the bare
            // `*()^N` as a lone target hits rustc's E0207 — shared
            // declaration, one used param).
            TySplat::Tuple(t) => {
                // Flatten own elements FIRST (groups/arrays expand —
                // `*(@u*)` = `*(u8,...,usize)`), then delegate: pow must
                // see the real element count (`*(@u*)^2` = Cartesian, not
                // pow_single on one group). Tuples stay intact (one-layer
                // semantics).
                let (elems, own_decl) =
                    splat_expand(Ty { span, kind: TyKind::Splat(TySplat::Tuple(t)) });
                let result = TyTuple(elems).apply(o, span);
                let Ty { span, kind } = result;
                let shaped = match kind {
                    TyKind::Tuple(t) => TySplat::Tuple(t).to_ty().with_span(span),
                    TyKind::Array(a) => {
                        // Pow Cartesian combos re-wrap into splats —
                        // `*(A,B)^2` = `[*(A,A), *(A,B), *(B,A), *(B,B)]`:
                        // each combo is a param-position list that a
                        // right-splat chain flattens into the container
                        // (`A^*(A,B)^2` = `A<A,A>`/`A<A,B>`/...). A lone
                        // target flattens to duplicates (E0119) — use
                        // `(A,B)^2` for tuple impls.
                        let combos =
                            a.0.into_iter()
                                .map(|t| match t.kind {
                                    TyKind::Tuple(tt) => {
                                        TySplat::Tuple(tt).to_ty().with_span(span)
                                    }
                                    _ => t,
                                })
                                .collect::<Vec<_>>();
                        Ty { span, kind: TyKind::Array(TyArray(combos)) }
                    }
                    TyKind::WithType(wt) => {
                        let inner = *wt.1;
                        if let TyKind::Tuple(t) = inner.kind {
                            TyWithType(wt.0, TySplat::Tuple(t).to_ty().into())
                                .to_ty()
                                .with_span(span)
                        } else {
                            TyWithType(wt.0, inner.into()).to_ty().with_span(span)
                        }
                    }
                    other => Ty { span, kind: other },
                };
                match own_decl {
                    Some(d) => TyWithType(d, shaped.into()).to_ty().with_span(span),
                    None => shaped,
                }
            }
        }
    }
}
