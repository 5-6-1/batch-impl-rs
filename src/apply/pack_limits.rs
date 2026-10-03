//! Bounds for the internal Pack engine, including direct-AST callers that do
//! not pass through the parser's nesting guard.

use proc_macro2::{Ident, Span};

use super::{check_expand_limit, check_expand_mass};
use crate::ast::*;
use crate::util::{MAX_NEST_DEPTH, depth_err};

pub(super) fn check_depth(depth: usize, span: Span) -> Option<Ty> {
    (depth > MAX_NEST_DEPTH).then(|| {
        TyError(depth_err(&[Ident::new("pack", span).into()], " in internal Pack evaluation"))
            .to_ty()
            .with_span(span)
    })
}

/// Moving traversal avoids cloning the very input whose depth/mass has not yet
/// been checked. It shares the AST's exhaustive child traversal authority.
pub(super) fn checked_input(value: Ty) -> Result<Ty, Ty> {
    checked_tree(value, "Pack input")
}

pub(super) fn checked_result(value: Ty) -> Ty {
    match checked_tree(value, "Pack result") {
        Ok(value) => value,
        Err(error) => error,
    }
}

fn checked_tree(value: Ty, what: &str) -> Result<Ty, Ty> {
    fn visit(value: Ty, what: &str, depth: usize, mass: &mut usize, error: &mut Option<Ty>) -> Ty {
        if error.is_some() {
            return value;
        }
        *mass = mass.saturating_add(1);
        *error = check_depth(depth, value.span).or_else(|| check_expand_mass(what, *mass));
        if error.is_some() {
            return value;
        }
        value.map_children(&mut |child| visit(child, what, depth + 1, mass, error))
    }
    let mut error = None;
    let value = visit(value, what, 0, &mut 0, &mut error);
    match error {
        Some(error) => Err(error),
        None => Ok(value),
    }
}

/// Bounds generation before tuple_pow duplicates any template. The cost of a
/// bound-generator slot includes both the reference and its fresh declaration.
pub(super) fn check_generation(slots: &[Ty], decl: Option<&TyTypeParam>, n: usize) -> Option<Ty> {
    if let Some(error) = check_expand_limit(&format!("tuple .{n}"), n) {
        return Some(error);
    }
    let mut mass = match slots {
        [] => {
            if n == 0 {
                1
            } else {
                2 + 2 * n
            }
        }
        [slot] => match &slot.kind {
            TyKind::TypeParam(_) if n > 0 => {
                n.saturating_mul(count_leaves(slot).saturating_add(1)).saturating_add(2)
            }
            _ => n.saturating_mul(count_leaves(slot)).saturating_add(1),
        },
        many => {
            let mut combinations = 1usize;
            for _ in 0..n {
                combinations = combinations.saturating_mul(many.len());
                if let Some(error) = check_expand_limit("(A, B).N", combinations) {
                    return Some(error);
                }
            }
            let mut plain_combinations = 1usize;
            let plain_slots =
                many.iter().filter(|slot| !matches!(slot.kind, TyKind::TypeParam(_))).count();
            for _ in 0..n {
                plain_combinations = plain_combinations.saturating_mul(plain_slots);
            }
            let weight = many
                .iter()
                .map(|slot| match &slot.kind {
                    TyKind::TypeParam(tp) => 1usize.saturating_add(
                        tp.params
                            .iter()
                            .map(|(_, b)| {
                                1usize.saturating_add(b.as_deref().map_or(0, count_leaves))
                            })
                            .fold(0usize, usize::saturating_add),
                    ),
                    _ => count_leaves(slot),
                })
                .fold(0usize, usize::saturating_add);
            let cells = if n == 0 { 0 } else { combinations / many.len() };
            1usize
                .saturating_add(combinations)
                .saturating_add(combinations.saturating_sub(plain_combinations))
                .saturating_add(n.saturating_mul(cells).saturating_mul(weight))
        }
    };
    if let Some(decl) = decl {
        // carry merges declarations into a root generator declaration; there
        // is only one WithType wrapper in those two generation forms.
        let shares_wrapper = n > 0 && matches!(slots, [] | [Ty { kind: TyKind::TypeParam(_), .. }]);
        let carrier_mass = count_leaves(&decl.clone().to_ty());
        mass = mass.saturating_add(carrier_mass.saturating_sub(usize::from(shares_wrapper)));
    }
    check_expand_mass("pack generation", mass)
}
