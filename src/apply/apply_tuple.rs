use quote::ToTokens;

use crate::apply::{Apply, check_expand_limit, err_ty, err_ty_at, expand_limit_err};
use crate::ast::*;
use crate::util::cartesian;
use proc_macro2::Span;

/// `N..M` / `N..=M`: calls f for every length n in the range, packing results into a list.
/// Empty range (`start >= end`) or over-limit (len > [`MAX_EXPAND`]): a typo diagnostic
pub(crate) fn map_range(
    start: usize, end: usize, inclusive: bool, span: Span, f: impl Fn(usize) -> Ty,
) -> Ty {
    let end_mark = if inclusive { "=" } else { "" };
    let ns = if inclusive {
        (start..=end).collect::<Vec<_>>()
    } else {
        (start..end).collect::<Vec<_>>()
    };
    if ns.is_empty() {
        return err_ty_at(
            &format!(
                "batch-impl: range `{}..{}{}` is empty (start not below end); no impls will be generated",
                start, end, end_mark
            ),
            span,
        );
    }
    if let Some(e) =
        check_expand_limit(&format!("range `{}..{}{}`", start, end, end_mark), ns.len())
    {
        return e;
    }
    TyArray(ns.into_iter().map(f).collect()).into()
}

/// `(...,).N`: expands the tuple to length N (empty / single / multi-element handled separately)
/// `N` above [`MAX_EXPAND`] is a typo diagnostic (covers `().N` / `(T,).N`).
fn tuple_pow(mut elems: Vec<Ty>, n: usize) -> Ty {
    if let Some(e) = check_expand_limit(&format!("tuple `.{}`", n), n) {
        return e;
    }
    match elems.len() {
        0 => pow_empty(n),
        // len == 1 is guaranteed by the match; the remove(0) out-of-bounds branch is unreachable
        1 => pow_single(elems.remove(0), n),
        _ => pow_cartesian(elems, n),
    }
}

/// `().N` => `<A,B,...,N>(A,B,...,N)` — generate N fresh generic params and wrap
fn pow_empty(n: usize) -> Ty {
    if n == 0 {
        return TyTuple(vec![]).into();
    }
    let g = take_group();
    let params = fresh_params(g, n);
    let tp = TyTypeParam {
        params: params.clone().into_iter().map(|p| (p.into(), None)).collect(),
        bindings: vec![],
    }
    .to_ty();
    tp.apply(TyTuple(params).into())
}

/// `(T,).N` => `(T,T,...,T)`; `(<Bound>).N` => `(A:Bound, B:Bound, ...)`
fn pow_single(template: Ty, n: usize) -> Ty {
    let template_span = template.span;
    if let TyKind::TypeParam(tp) = template.kind.clone() {
        // From `(<Bound>).N`: exactly one unbound param (guaranteed by parse_angle_bracket_contents)
        if tp.params.len() != 1 || tp.params[0].1.is_some() {
            return err_ty(
                "batch-impl: unexpected bound parameter in (<Trait>)⁁; this is an internal error",
            );
        }
        let g = take_group();
        let params = fresh_params(g, n);
        let bound_ty = *tp.params[0].0.clone();
        return TyTypeParam {
            params: params
                .clone()
                .into_iter()
                .map(|p| (p.into(), Some(bound_ty.clone())))
                .collect(),
            bindings: vec![],
        }
        .to_ty()
        .with_span(template_span)
        .apply(TyTuple(params).into());
    }
    TyTuple((0..n).map(|_| Ty { span: template_span, kind: template.kind.clone() }).collect())
        .into()
}

/// `(A,B,..).N`: N-way Cartesian product, choosing one of all elements per position.
/// The product count is checked inside `cartesian` before each allocation
/// (`elems.N` can far exceed [`MAX_EXPAND`]).
fn pow_cartesian(elems: Vec<Ty>, n: usize) -> Ty {
    let dims: Vec<Vec<Ty>> = std::iter::repeat_n(elems, n).collect();
    let combos = match cartesian(&dims, MAX_EXPAND) {
        Ok(c) => c,
        Err(size) => return expand_limit_err("tuple Cartesian product", size),
    };
    TyArray(combos.into_iter().map(instantiate_combo).collect()).into()
}

/// Instantiate one Cartesian combination: TypeParam positions get a fresh param with the bound
/// preserved; other positions stay as-is
fn instantiate_combo(elems: Vec<Ty>) -> Ty {
    let g = take_group();
    let mut tuple_elems = vec![];
    let mut param_decls = vec![];
    let mut pos = 0;
    for elem in elems {
        let elem_span = elem.span;
        match elem.kind {
            TyKind::TypeParam(tp) => {
                let name = fresh_param(g, pos);
                pos += 1;
                // Keep the original bound (previously the param name was mistaken for the bound;
                // `(A: Clone, T).N` produced `_Param: A` instead of `_Param: Clone`)
                let params = tp
                    .params
                    .iter()
                    .map(|(_, bound)| (TyPrimitive(name.clone()).to_ty().into(), bound.clone()))
                    .collect();
                param_decls.push(TyTypeParam { params, bindings: vec![] });
                tuple_elems.push(TyPrimitive(name).to_ty().with_span(elem_span));
            }
            _ => tuple_elems.push(Ty { span: elem_span, kind: elem.kind }),
        }
    }
    let tuple = TyTuple(tuple_elems).into();
    if param_decls.is_empty() {
        return tuple;
    }
    let merged = param_decls.into_iter().fold(
        TyTypeParam { params: vec![], bindings: vec![] },
        |mut acc, tp| {
            acc.extend(tp);
            acc
        },
    );
    merged.to_ty().apply(tuple)
}

fn fresh_params(g: usize, n: usize) -> Vec<Ty> {
    (0..n).map(|i| TyPrimitive(fresh_param(g, i)).into()).collect()
}

impl Apply for TyTuple {
    /// `(A,B,).C` => append C; `(A,).N` => tuple-length expansion; `(A,).N..M` => range expansion
    fn apply_help(mut self, o: Ty, span: Span) -> Ty {
        match o.kind {
            TyKind::Num(TyNum(n)) => tuple_pow(self.0, n),
            _ => {
                self.0.push(o);
                self.to_ty().with_span(span)
            }
        }
    }
}

impl Apply for TyGroup {
    /// `(T)` strips to the inner type, so `(T).N` equals `T.N` (e.g.
    /// `(W).2` = `W.2` = `W<2>`, a const-generic argument; only valid for
    /// types with a const generic — `(u8).2` would emit `u8<2>`, which
    /// rustc rejects with E0109). Tuple generation needs `(T,).N`.
    /// A bare `<T>` group is rejected earlier by parsing (`<` right after
    /// `(` is not a type).
    fn apply_help(self, o: Ty, _span: Span) -> Ty {
        self.0.apply(o)
    }
}

impl Apply for TyFn {
    /// `fn.(A,B)` => `fn(A,B)` (fills in params); `fn(A,B)-C` => `fn(A,B)->C` (adds return type).
    /// The `is_unsafe` field passes through (`unsafe fn.(A,B)` => `unsafe fn(A,B)`).
    fn apply_help(self, o: Ty, span: Span) -> Ty {
        match self {
            // A bare fn / Fn-trait gets its params via `.`; the right side must be a
            // tuple (a Group like `fn.((i8,i16))` is unwrapped by the default apply's
            // Group branch; here `o` is always plain).
            TyFn(None, None, is_unsafe, kind) => match o.kind {
                TyKind::Tuple(t) => {
                    TyFn(t.0.into(), None, is_unsafe, kind).to_ty().with_span(span)
                }
                _ => err_ty_at(
                    "batch-impl: the right side of the `fn`/`Fn` prefix must be a tuple type, e.g. fn.(i32, u32)",
                    span,
                ),
            },
            // Has params: append the return type (the space/`.` application)
            TyFn(Some(params), None, is_unsafe, kind) => {
                TyFn(params.into(), o.into(), is_unsafe, kind).to_ty().with_span(span)
            }
            TyFn(Some(_), Some(_), _, _) => err_ty_at(
                "batch-impl: the `fn` type already has a return type; cannot apply again",
                span,
            ),
            // Impossible: params None but return Some
            TyFn(None, Some(_), _, _) => err_ty_at(
                "batch-impl: the `fn` type is missing a parameter list; internal error",
                span,
            ),
        }
    }
}

impl Apply for TyWithAttr {
    /// `#[attr].T` => `#[attr] T` (attaches the attribute to the type);
    /// with an inner already attached, the operator applies to the inner
    /// (`#[attr] Box.u8` = `#[attr] Box<u8>` — 0.7.2 fix: the inner was
    /// silently replaced).
    fn apply_help(self, o: Ty, span: Span) -> Ty {
        let inner = match self.1 {
            Some(t) => t.apply(o),
            None => o,
        };
        TyWithAttr(self.0, inner.into()).to_ty().with_span(span)
    }
}

impl Apply for TyTypeParam {
    /// `<T>.U` => `WithType(<T>, U)` (generic parameters applied to the target type)
    fn apply_help(self, o: Ty, span: Span) -> Ty {
        TyWithType(self, o.into()).to_ty().with_span(span)
    }
}
impl Apply for TyNum {
    /// A number cannot be a left operand (used only on the right, e.g. `T.3`)
    fn apply_help(self, _: Ty, span: Span) -> Ty {
        err_ty_at(
            &format!(
                "batch-impl: number `{}` cannot be a left operand; use it on the right (e.g. T.{})",
                self.0, self.0
            ),
            span,
        )
    }
}
impl Apply for TyRange {
    /// A range cannot be a left operand (used only on the right, e.g. `T.1..3`)
    fn apply_help(self, _: Ty, span: Span) -> Ty {
        let end_mark = if self.inclusive { "=" } else { "" };
        err_ty_at(
            &format!(
                "batch-impl: range `{}..{}{}` cannot be a left operand; it goes on the right (e.g. T.{}..{}{})",
                self.start, self.end, end_mark, self.start, self.end, end_mark
            ),
            span,
        )
    }
}
impl Apply for TyPrimitiveArray {
    /// `[].T` => `[T]` (empty base wraps a slice); `[T].N` => `[T; N]` (fixed-size array)
    ///
    /// The length right side can be a numeric literal (`[u8].3`), a const generic (`[u8].N`), or a
    /// list/range (expanded item-wise by the top-level right-operand dispatch); re-applying to
    /// a finished array is an error.
    fn apply_help(self, o: Ty, span: Span) -> Ty {
        match (self.0, self.1) {
            (None, None) => TyPrimitiveArray(o.into(), None).to_ty().with_span(span),
            (Some(elem), None) => {
                TyPrimitiveArray(elem.into(), o.to_token_stream().into()).to_ty().with_span(span)
            }
            _ => err_ty_at("batch-impl: fixed-size array `[T; N]` cannot be a left operand", span),
        }
    }
}

/// Macro generating the "passthrough to the inner type then re-wrap" apply impls for the four
/// wrapper kinds (WithTrait/WithType always have an inner type, WithCode/WithWhere have an optional
/// one); their apply_help bodies are isomorphic, folded into a macro: adding a wrapper is one line,
/// and the "outer apply passes through to the inner target" semantics is declared once at the macro
/// definition site. Optional-inner: with `None`, the right operand directly takes the inner slot.
/// Note: `self.1` is written in the macro body (definition-site hygiene), not passable as
/// a macro argument — argument tokens keep the call-site context, and `self` would resolve to the
/// module's self (E0424).
macro_rules! impl_apply_optional_inner {
    ($ty:ident, $variant:ident) => {
        impl Apply for $ty {
            fn apply_help(self, o: Ty, span: Span) -> Ty {
                let inner = match self.0 {
                    Some(t) => t.apply(o),
                    None => o,
                };
                $ty(inner.into(), self.1).to_ty().with_span(span)
            }
        }
    };
}
/// Always-inner form: apply the right operand to the inner type, then re-wrap.
macro_rules! impl_apply_inner {
    ($ty:ident, $variant:ident) => {
        impl Apply for $ty {
            fn apply_help(self, o: Ty, span: Span) -> Ty {
                $ty(self.0, self.1.apply(o).into()).to_ty().with_span(span)
            }
        }
    };
}

impl_apply_inner!(TyWithTrait, WithTrait);
impl_apply_inner!(TyWithType, WithType);
impl_apply_optional_inner!(TyWithCode, WithCode);
impl_apply_optional_inner!(TyWithWhere, WithWhere);
impl_apply_optional_inner!(TyWithImpl, WithImpl);
