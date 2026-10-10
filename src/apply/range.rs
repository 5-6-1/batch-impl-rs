//! Range alternatives keep the identities of their generating source nodes.

use std::collections::HashMap;

use proc_macro2::Span;

use super::apply_tuple::{PowScope, map_range, tuple_pow_with};
use super::check_expand_mass;
use crate::ast::*;

pub(super) fn apply(left: Ty, range: TyRange, span: Span) -> Ty {
    let mut sources = Sources { groups: HashMap::new(), path: vec![] };
    let mapped = map_range(range.start, range.end, range.inclusive, span, |n| {
        sources.number(left.clone(), n)
    });
    if let TyKind::Array(array) = &mapped.kind
        && let Some(error) =
            check_expand_mass("range chain expansion", array.0.iter().map(count_leaves).sum())
    {
        return error;
    }
    mapped
}

/// An Array edge identifies a distinct source. Single-child apply wrappers
/// are transparent; neither Cartesian output count nor arity changes this
/// route. No identity is reconstructed from display names or source spans.
struct Sources {
    groups: HashMap<Vec<usize>, usize>,
    path: Vec<usize>,
}

impl Sources {
    fn group(&mut self, slots: &[Ty]) -> Option<usize> {
        if slots.is_empty() || slots.iter().any(|slot| matches!(slot.kind, TyKind::TypeParam(_))) {
            Some(*self.groups.entry(self.path.clone()).or_insert_with(take_group))
        } else {
            None
        }
    }

    fn tuple(&mut self, slots: Vec<Ty>, n: usize) -> Ty {
        let group = self.group(&slots);
        tuple_pow_with(slots, n, PowScope::Range(group))
    }

    fn number(&mut self, value: Ty, n: usize) -> Ty {
        let Ty { span, kind } = value;
        match kind {
            TyKind::Array(array) => {
                let mut out = vec![];
                for (index, value) in array.0.into_iter().enumerate() {
                    self.path.push(index);
                    out.push(self.number(value, n));
                    self.path.pop();
                }
                TyArray(out).to_ty().with_span(span)
            }
            TyKind::Group(group) => self.number(*group.0, n),
            TyKind::Tuple(tuple) => self.tuple(tuple.0, n),
            TyKind::Pack(pack) => {
                super::pack::range_number(pack.to_ty().with_span(span), n, span, |slots| {
                    self.group(slots)
                })
            }
            TyKind::Fn(TyFn(Some(params), None, unsafe_, kind)) => {
                let generated = self.tuple(params, n);
                TyFn(None, None, unsafe_, kind).to_ty().with_span(span).apply(generated)
            }
            TyKind::WithType(w) => {
                TyWithType(w.0, self.number(*w.1, n).into()).to_ty().with_span(span)
            }
            TyKind::WithTrait(w) => {
                TyWithTrait(w.0, self.number(*w.1, n).into()).to_ty().with_span(span)
            }
            TyKind::WithDyn(w) => {
                TyWithDyn(self.number(*w.0, n).into(), w.1).to_ty().with_span(span)
            }
            TyKind::WithFor(w) => {
                TyWithFor(w.0, self.number(*w.1, n).into()).to_ty().with_span(span)
            }
            TyKind::Prefixed(w) => {
                TyPrefixed(w.0, self.number(*w.1, n).into()).to_ty().with_span(span)
            }
            TyKind::WithPrefix(TyWithPrefix(prefix, Some(inner)))
                if !matches!(prefix, TyPrefix::SelfType) =>
            {
                TyWithPrefix(prefix, self.number(*inner, n).into()).to_ty().with_span(span)
            }
            TyKind::WithAttr(TyWithAttr(attr, Some(inner))) => {
                TyWithAttr(attr, self.number(*inner, n).into()).to_ty().with_span(span)
            }
            TyKind::WithCode(TyWithCode(Some(inner), code)) => {
                TyWithCode(self.number(*inner, n).into(), code).to_ty().with_span(span)
            }
            TyKind::WithWhere(TyWithWhere(Some(inner), clause)) => {
                TyWithWhere(self.number(*inner, n).into(), clause).to_ty().with_span(span)
            }
            TyKind::WithImpl(TyWithImpl(Some(inner), template)) => {
                TyWithImpl(self.number(*inner, n).into(), template).to_ty().with_span(span)
            }
            kind => Ty { span, kind }.apply(TyNum(n).to_ty().with_span(span)),
        }
    }
}
