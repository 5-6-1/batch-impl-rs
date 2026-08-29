//! Bound-generator distribution: a generator **range** inside an impl-generic
//! bound (`<T: Fn.().0..4 R>`) expands to a `TyArray` at the apply layer (one
//! element per arity). Without distribution the array would render as an
//! illegal bound (`T: [A, B, ...]`); instead each element becomes its own
//! impl with the bound pinned to that element's arity — exactly the
//! "arity 0..4 → one impl per arity" semantics. The fresh params inside each
//! element (`WithType(<P0,P1>, Fn(P0,P1) -> R)`) are hoisted to the impl
//! generics by the later `hoist_bound_fresh` pass, and the target's `@0..`
//! range re-opens against that impl's own fresh list at render (each
//! distributed impl sweeps its names independently).

use crate::ast::TyKind;
use crate::codegen::extract::ImplParts;
use crate::util::cartesian;

/// Splits `parts` into one `ImplParts` per bound-array element (the Cartesian
/// product when several bounds are ranges). With no array bounds, returns the
/// single input unchanged. An over-limit product falls back to the single
/// input: the render layer reports the size against [`crate::ast::MAX_EXPAND`]
/// instead of silently truncating.
pub(crate) fn distribute_bound_arrays(parts: ImplParts) -> Vec<ImplParts> {
    let array_at = |i: usize| match &parts.impl_generics[i].1 {
        Some(t) if matches!(&t.kind, TyKind::Array(_)) => match &t.kind {
            TyKind::Array(a) => Some(a.0.clone()),
            _ => None,
        },
        _ => None,
    };
    // Collect (position, elements) in one pass — no `.is_some()` filter
    // followed by a second `array_at(i).unwrap()` (check + extraction in
    // one step).
    let dims: Vec<(usize, Vec<_>)> = (0..parts.impl_generics.len())
        .filter_map(|i| array_at(i).map(|elems| (i, elems)))
        .collect();
    if dims.is_empty() {
        return vec![parts];
    }
    let positions: Vec<usize> = dims.iter().map(|(i, _)| *i).collect();
    let combos = match cartesian(
        &dims.into_iter().map(|(_, elems)| elems).collect::<Vec<_>>(),
        crate::ast::MAX_EXPAND,
    ) {
        Ok(c) => c,
        Err(_) => {
            // Over-limit product: do NOT fall back to the single input — the
            // array bound would render as `T: [A, B, ...]` (an illegal bound
            // rustc reports with a confusing error; the render layer has no
            // size check). Emit a targeted diagnostic instead, through the
            // error-bound channel the driver aggregates.
            let mut p = parts;
            let total = positions
                .iter()
                .map(|&i| match &p.impl_generics[i].1 {
                    Some(t) if matches!(&t.kind, TyKind::Array(_)) => {
                        if let TyKind::Array(a) = &t.kind { a.0.len() } else { 0 }
                    }
                    _ => 0,
                })
                .product::<usize>();
            p.impl_generics[positions[0]].1 = Some(crate::apply::err_ty(&format!(
                "batch-impl: bound-generator distribution expands to {total} \
                     impls (limit {}); reduce the range sizes",
                crate::ast::MAX_EXPAND,
            )));
            return vec![p];
        }
    };
    combos
        .into_iter()
        .map(|combo| {
            let mut p = parts.clone();
            for (&i, elem) in positions.iter().zip(combo) {
                p.impl_generics[i].1 = Some(elem);
            }
            p
        })
        .collect()
}
