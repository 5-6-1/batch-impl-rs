//! Pack application kernel. Mapping preserves direct right rows; ordinary
//! hosts consume the resulting packs later in `ast::materialize`.

use proc_macro2::Span;

use super::pack_limits::{check_depth, check_generation, checked_input, checked_result};
use super::{Apply, apply_tuple::tuple_pow, check_expand_mass};
use crate::ast::*;

/// Opens only a value's direct container layer. A normal tuple or candidate
/// nested inside that layer remains an ordinary type/candidate node.
pub(crate) fn packify(value: Ty) -> Ty {
    match checked_input(value) {
        Ok(value) => checked_result(packify_inner(value)),
        Err(error) => error,
    }
}

fn packify_inner(value: Ty) -> Ty {
    let Ty { span, kind } = value;
    match kind {
        TyKind::Group(g) => packify_inner(*g.0),
        TyKind::WithType(w) => carry(w.0, packify_inner(*w.1), span),
        TyKind::Pack(p) => p.to_ty().with_span(span),
        TyKind::Tuple(t) => TyPack(t.0).to_ty().with_span(span),
        TyKind::Array(a) => TyPack(a.0).to_ty().with_span(span),
        other => TyPack(vec![Ty { span, kind: other }]).to_ty().with_span(span),
    }
}

/// Keeps declarations independently of the result's used names, including
/// empty packs. Bounds and bindings are not deduplicated here: fresh identity
/// merging belongs to the existing codegen declaration pass.
fn carry(mut params: TyTypeParam, value: Ty, span: Span) -> Ty {
    match value.kind {
        TyKind::WithType(inner) => {
            params.extend(inner.0);
            carry(params, *inner.1, span)
        }
        kind => TyWithType(params, Ty { span: value.span, kind }.into()).to_ty().with_span(span),
    }
}

/// Maps a container incrementally, stopping before accumulating an unbounded
/// tree. Candidate/pack identity is chosen by the caller, never inferred from
/// the mapped members.
fn mapped(
    items: Vec<Ty>, span: Span, build: impl FnOnce(Vec<Ty>) -> Ty, mut f: impl FnMut(Ty) -> Ty,
) -> Ty {
    let mut out = vec![];
    let mut mass = 1usize;
    for item in items {
        let result = f(item);
        mass = mass.saturating_add(count_leaves(&result));
        if let Some(error) = check_expand_mass("pack mapping", mass) {
            return error.with_span(span);
        }
        out.push(result);
    }
    build(out).with_span(span)
}

/// Continues one mapping task with the *whole* current right row. Selecting a
/// left candidate that happens to be a pack must not reopen that right row as
/// a new pairing operation. Group/declaration wrappers are transparent here.
pub(crate) fn map_task(left: Ty, right: Ty) -> Ty {
    let left = match checked_input(left) {
        Ok(left) => left,
        Err(error) => return error,
    };
    let right = match checked_input(right) {
        Ok(right) => right,
        Err(error) => return error,
    };
    checked_result(run_task(left, right, 0))
}

fn run_task(left: Ty, right: Ty, depth: usize) -> Ty {
    let span = left.span;
    if let Some(error) = check_depth(depth, span) {
        return error;
    }
    match left.kind {
        TyKind::Group(g) => return run_task(*g.0, right, depth + 1),
        TyKind::WithType(w) => return carry(w.0, run_task(*w.1, right, depth + 1), span),
        _ => {}
    }
    match right.kind {
        TyKind::Error(error) => return Ty { span: right.span, kind: TyKind::Error(error) },
        TyKind::Group(g) => return run_task(left, *g.0, depth + 1),
        TyKind::WithType(w) => return carry(w.0, run_task(left, *w.1, depth + 1), span),
        TyKind::WithCode(w) => {
            let inner = match w.0 {
                Some(r) => run_task(left, *r, depth + 1),
                None => left,
            };
            return TyWithCode(inner.into(), w.1).to_ty().with_span(span);
        }
        TyKind::WithWhere(w) => {
            let inner = match w.0 {
                Some(r) => run_task(left, *r, depth + 1),
                None => left,
            };
            return TyWithWhere(inner.into(), w.1).to_ty().with_span(span);
        }
        TyKind::WithImpl(w) => {
            let inner = match w.0 {
                Some(r) => run_task(left, *r, depth + 1),
                None => left,
            };
            return TyWithImpl(inner.into(), w.1).to_ty().with_span(span);
        }
        TyKind::Array(a) => {
            return mapped(
                a.0,
                span,
                |xs| TyArray(xs).to_ty(),
                |r| run_task(left.clone(), r, depth + 1),
            );
        }
        _ => {}
    }
    match left.kind {
        TyKind::Array(a) => {
            mapped(a.0, span, |xs| TyArray(xs).to_ty(), |l| run_task(l, right.clone(), depth + 1))
        }
        TyKind::Pack(p) => {
            mapped(p.0, span, |xs| TyPack(xs).to_ty(), |l| run_task(l, right.clone(), depth + 1))
        }
        // Attached metadata follows the same passthrough direction as ordinary
        // apply, but must preserve this mapping continuation around its target.
        TyKind::WithTrait(w) => {
            TyWithTrait(w.0, run_task(*w.1, right, depth + 1).into()).to_ty().with_span(span)
        }
        TyKind::Prefixed(p) => {
            TyPrefixed(p.0, run_task(*p.1, right, depth + 1).into()).to_ty().with_span(span)
        }
        TyKind::WithPrefix(TyWithPrefix(prefix, Some(inner)))
            if !matches!(prefix, TyPrefix::SelfType) =>
        {
            TyWithPrefix(prefix, run_task(*inner, right, depth + 1).into()).to_ty().with_span(span)
        }
        TyKind::WithDyn(w) => {
            TyWithDyn(run_task(*w.0, right, depth + 1).into(), w.1).to_ty().with_span(span)
        }
        TyKind::WithFor(w) => {
            TyWithFor(w.0, run_task(*w.1, right, depth + 1).into()).to_ty().with_span(span)
        }
        TyKind::WithAttr(TyWithAttr(attr, Some(inner))) => {
            TyWithAttr(attr, run_task(*inner, right, depth + 1).into()).to_ty().with_span(span)
        }
        TyKind::WithCode(TyWithCode(Some(inner), body)) => {
            TyWithCode(run_task(*inner, right, depth + 1).into(), body).to_ty().with_span(span)
        }
        TyKind::WithWhere(TyWithWhere(Some(inner), clause)) => {
            TyWithWhere(run_task(*inner, right, depth + 1).into(), clause).to_ty().with_span(span)
        }
        TyKind::WithImpl(TyWithImpl(Some(inner), template)) => {
            TyWithImpl(run_task(*inner, right, depth + 1).into(), template).to_ty().with_span(span)
        }
        kind => Ty { span, kind }.apply(right),
    }
}

/// Numeric generation explicitly consumes nested packs and declaration
/// carriers. Ordinary candidates and tuples remain intact template slots.
fn numeric_slots(value: Ty, slots: &mut Vec<Ty>, decl: &mut Option<TyTypeParam>) {
    match value.kind {
        TyKind::Group(g) => numeric_slots(*g.0, slots, decl),
        TyKind::WithType(w) => {
            *decl = merge_decls(decl.take(), Some(w.0));
            numeric_slots(*w.1, slots, decl);
        }
        TyKind::Pack(p) => {
            for item in p.0 {
                numeric_slots(item, slots, decl);
            }
        }
        _ => slots.push(value),
    }
}

/// Changes only the result containers produced by tuple generation; applying
/// `packify` to a candidate result would accidentally collect its branches.
fn generated_pack(value: Ty) -> Ty {
    let Ty { span, kind } = value;
    match kind {
        TyKind::Tuple(t) => TyPack(t.0).to_ty().with_span(span),
        TyKind::Array(a) => {
            TyArray(a.0.into_iter().map(generated_pack).collect()).to_ty().with_span(span)
        }
        TyKind::WithType(w) => carry(w.0, generated_pack(*w.1), span),
        other => Ty { span, kind: other },
    }
}

impl Apply for TyPack {
    fn apply_help(self, right: Ty, span: Span) -> Ty {
        let left = match checked_input(self.to_ty().with_span(span)) {
            Ok(left) => left,
            Err(error) => return error,
        };
        let right = match checked_input(right) {
            Ok(right) => right,
            Err(error) => return error,
        };
        match right.kind {
            TyKind::Num(TyNum(n)) => {
                let mut slots = vec![];
                let mut decl = None;
                numeric_slots(left, &mut slots, &mut decl);
                if let Some(error) = check_generation(&slots, decl.as_ref(), n) {
                    return error.with_span(span);
                }
                let generated = generated_pack(tuple_pow(slots, n)).with_span(span);
                let result = match decl {
                    Some(d) => carry(d, generated, span),
                    None => generated,
                };
                checked_result(result)
            }
            TyKind::Pack(rows) => checked_result(mapped(
                rows.0,
                span,
                |xs| TyPack(xs).to_ty(),
                |row| run_task(left.clone(), row, 0),
            )),
            kind => map_task(left, Ty { span: right.span, kind }),
        }
    }
}
