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

use proc_macro2::TokenStream;

use crate::ast::TyKind;
use crate::codegen::extract::ImplParts;
use crate::util::{cartesian, compile_error_str};

/// Splits `parts` into one `ImplParts` per bound-array element (the Cartesian
/// product when several bounds are ranges). With no array bounds, returns the
/// single input unchanged. An over-limit product is an `Err` carrying the
/// diagnostic: the whole expansion is replaced by the message instead of
/// emitting a malformed impl. Neither alternative is acceptable — falling back
/// to the single input renders the array bound as `T: [A, B, ...]` (an illegal
/// bound rustc reports confusingly), and placing the diagnostic *inside* the
/// bound position is worse: the item-form `compile_error!` carries a trailing
/// `;`, a syntax error that buries the message under parser fallout.
pub(crate) fn distribute_bound_arrays(parts: ImplParts) -> Result<Vec<ImplParts>, TokenStream> {
    let array_at = |i: usize| match parts.impl_generics.get(i).and_then(|(_, b)| b.as_ref()) {
        Some(t) => match &t.kind {
            TyKind::Array(a) => Some(a.0.clone()),
            _ => None,
        },
        None => None,
    };
    // Collect (position, elements) in one pass — no `.is_some()` filter
    // followed by a second `array_at(i).unwrap()` (check + extraction in
    // one step).
    let dims: Vec<(usize, Vec<_>)> = (0..parts.impl_generics.len())
        .filter_map(|i| array_at(i).map(|elems| (i, elems)))
        .collect();
    if dims.is_empty() {
        return Ok(vec![parts]);
    }
    let positions: Vec<usize> = dims.iter().map(|(i, _)| *i).collect();
    let combos = match cartesian(
        &dims.into_iter().map(|(_, elems)| elems).collect::<Vec<_>>(),
        crate::ast::MAX_EXPAND,
    ) {
        Ok(c) => c,
        // `cartesian` rejects before allocating and reports the would-be
        // product, which is exactly the count the diagnostic needs.
        Err(size) => {
            let span = positions
                .first()
                .and_then(|&i| parts.impl_generics.get(i))
                .and_then(|(_, b)| b.as_ref())
                .map_or_else(proc_macro2::Span::call_site, |b| b.span);
            return Err(compile_error_str(
                &format!(
                    "batch-impl: bound-generator distribution expands to {size} impls \
                     (limit {}); reduce the range sizes",
                    crate::ast::MAX_EXPAND
                ),
                span,
            ));
        }
    };
    Ok(combos
        .into_iter()
        .map(|combo| {
            let mut p = parts.clone();
            for (&i, elem) in positions.iter().zip(combo) {
                let Some(slot) = p.impl_generics.get_mut(i) else {
                    continue;
                };
                slot.1 = Some(elem);
            }
            p
        })
        .collect())
}
