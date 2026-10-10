//! Tuple-length generation with fixed or source-stable range identities.

use crate::apply::{check_expand_limit, err_ty, expand_limit_err};
use crate::ast::*;
use crate::util::cartesian;

/// `(...,).N`: expands the tuple to length N (empty / single / multi-element handled separately)
/// `N` above [`MAX_EXPAND`] is a typo diagnostic (covers `().N` / `(T,).N`).
pub(super) fn tuple_pow(elems: Vec<Ty>, n: usize) -> Ty {
    tuple_pow_with(elems, n, PowScope::Fixed)
}

#[derive(Clone, Copy)]
pub(super) enum PowScope {
    Fixed,
    /// A range's non-generating templates have no fresh scope to declare.
    Range(Option<usize>),
}

impl PowScope {
    fn group(self) -> Option<usize> {
        match self {
            Self::Fixed => None,
            Self::Range(group) => group,
        }
    }
}

pub(super) fn tuple_pow_with(mut elems: Vec<Ty>, n: usize, source: PowScope) -> Ty {
    if let Some(e) = check_expand_limit(&format!("tuple .{}", n), n) {
        return e;
    }
    match elems.len() {
        0 => pow_empty(n, source.group()),
        // len == 1 is guaranteed by the match; the remove(0) out-of-bounds branch is unreachable
        1 => pow_single(elems.remove(0), n, source.group()),
        _ => pow_cartesian(elems, n, source),
    }
}

/// `().N` => `<A,B,...,N>(A,B,...,N)` — generate N fresh generic params and wrap
fn pow_empty(n: usize, source: Option<usize>) -> Ty {
    if n == 0 && source.is_none() {
        return TyTuple(vec![]).into();
    }
    let g = source.unwrap_or_else(take_group);
    let params = fresh_params(g, n);
    let tp = TyTypeParam {
        params: params.clone().into_iter().map(|p| (Box::new(p), None)).collect(),
        bindings: vec![],
        fresh_groups: source.into_iter().collect(),
    }
    .to_ty();
    tp.apply(TyTuple(params).into())
}

/// `(T,).N` => `(T,T,...,T)`; `(<Bound>).N` => `(A:Bound, B:Bound, ...)`
fn pow_single(template: Ty, n: usize, source: Option<usize>) -> Ty {
    let template_span = template.span;
    if let TyKind::TypeParam(tp) = template.kind.clone() {
        // From `(<Bound>).N`: exactly one unbound param (guaranteed by parse_angle_bracket_contents)
        if tp.params.len() != 1 || tp.params.first().is_some_and(|(_, b)| b.is_some()) {
            return err_ty(
                "batch-impl: a bound cannot ride on a repeated parameter — \
                 `(<T: Clone>).N` rewrites the template into N fresh parameters, so the \
                 bound would have to be copied with it; write the bound where the parameter \
                 is used instead",
            );
        }
        if n == 0 && source.is_none() {
            return TyTuple(vec![]).into();
        }
        let g = source.unwrap_or_else(take_group);
        let params = fresh_params(g, n);
        let bound_ty = match tp.params.first() {
            Some((name, _)) => *name.clone(),
            None => {
                return err_ty(
                    "batch-impl: internal error — the `(<Trait>)⁁` declaration has \
                     no parameter",
                );
            }
        };
        return TyTypeParam {
            params: params
                .clone()
                .into_iter()
                .map(|p| (Box::new(p.clone()), Some(Box::new(bound_ty.clone()))))
                .collect(),
            bindings: vec![],
            fresh_groups: source.into_iter().collect(),
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
fn pow_cartesian(elems: Vec<Ty>, n: usize, source: PowScope) -> Ty {
    let dims = std::iter::repeat_n(elems, n).collect::<Vec<_>>();
    let combos = match cartesian(&dims, MAX_EXPAND) {
        Ok(c) => c,
        Err(size) => return expand_limit_err("(A, B).N", size),
    };
    TyArray(combos.into_iter().map(|elems| instantiate_combo(elems, source)).collect()).into()
}

/// Instantiate one Cartesian combination: TypeParam positions get a fresh param with the bound
/// preserved; other positions stay as-is
fn instantiate_combo(elems: Vec<Ty>, source: PowScope) -> Ty {
    let mut g = match source {
        PowScope::Range(group) => group,
        PowScope::Fixed if elems.is_empty() => None,
        PowScope::Fixed => Some(take_group()),
    };
    let mut tuple_elems = vec![];
    let mut param_decls = vec![];
    let mut pos = 0;
    for elem in elems {
        let elem_span = elem.span;
        match elem.kind {
            TyKind::TypeParam(tp) => {
                let g = *g.get_or_insert_with(take_group);
                // The fresh reference and its declaration share one identity:
                // the structured (group, position) pair — the tuple element
                // is the structured reference node, the declaration carries
                // the same pair, so hoisting can dedup by identity alone.
                let name = fresh_ref_ty(g, pos);
                pos += 1;
                // Keep the original bound (previously the param name was mistaken for the bound;
                // `(A: Clone, T).N` produced `_Param: A` instead of `_Param: Clone`)
                let params = tp
                    .params
                    .iter()
                    .map(|(_, bound)| (Box::new(name.clone()), bound.clone()))
                    .collect();
                param_decls.push(TyTypeParam { params, bindings: vec![], fresh_groups: vec![] });
                tuple_elems.push(name.with_span(elem_span));
            }
            _ => tuple_elems.push(Ty { span: elem_span, kind: elem.kind }),
        }
    }
    let tuple = TyTuple(tuple_elems).into();
    if param_decls.is_empty() && source.group().is_none() {
        return tuple;
    }
    let merged = param_decls.into_iter().fold(
        TyTypeParam {
            params: vec![],
            bindings: vec![],
            fresh_groups: source.group().into_iter().collect(),
        },
        |mut acc, tp| {
            acc.extend(tp);
            acc
        },
    );
    merged.to_ty().apply(tuple)
}

/// One fresh **reference** node per generator position — the structured
/// carrier (`TyKind::Fresh`) that renders to the self-delimiting `@{g_i}`
/// token form. The same nodes serve as the declaration names inside the
/// `WithType` wrapper, so reference and declaration share one identity.
fn fresh_params(g: usize, n: usize) -> Vec<Ty> {
    (0..n).map(|i| fresh_ref_ty(g, i)).collect()
}

fn fresh_ref_ty(g: usize, i: usize) -> Ty {
    TyFresh(FreshRef { group: Some(g), start: i, end: FreshEnd::Single }).to_ty()
}
