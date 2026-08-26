//! `Ty` traversal: parallel expansion ([`Expand`]) and the single child-map
//! home ([`Ty::map_children`]). Split from `types.rs` so node definitions and
//! traversal stay under the per-file budget.

use crate::ast::types::{
    Ty, TyArray, TyBoundList, TyFn, TyGeneric, TyGroup, TyKind, TyParams, TyPrimitiveArray,
    TyTrait, TyTuple, TyTypeParam, TyWithAttr, TyWithCode, TyWithDyn, TyWithFor, TyWithImpl,
    TyWithPrefix, TyWithTrait, TyWithType, TyWithWhere,
};

pub(crate) enum Expand {
    Leaf(Ty),
    Many(Vec<Ty>),
}

/// Splat consumption: flatten a splat element (or any container / generator)
/// into its element list, hoisting fresh declarations out. Returns the flat
/// elements plus the merged declaration (if any generator was flattened — the
/// caller wraps the enclosing container in `WithType(decl, ...)`).
///
/// Shared by the parse layer (container element collection) and the apply
/// layer (right-splat argument appending / left-splat distribution).
pub(crate) fn splat_expand(ty: Ty) -> (Vec<Ty>, Option<TyTypeParam>) {
    match ty.kind {
        TyKind::Splat(s) => fold_splat_elems(s.elems().to_vec()),
        TyKind::Array(a) => fold_splat_elems(a.0),
        // Splat expands ONE layer: tuples are types, so they stay as single
        // elements — `*((a,b),)` = `(a,b)` (one tuple impl), and a tuple
        // inside a splat (`*(a,(b,c))`) keeps `(b,c)` intact. Only lists
        // (arrays, nested splats) and generators flatten.
        TyKind::Tuple(t) => (vec![Ty { span: ty.span, kind: TyKind::Tuple(t) }], None),
        TyKind::Group(g) => splat_expand(*g.0),
        // Generator: its inner container is a *param list* (the fresh tuple),
        // not a type — flatten it even though bare tuples stay single
        // elements (`(*(().3))` = `(P0,P1,P2)`, not `((P0,P1,P2),)`).
        TyKind::WithType(wt) => {
            let TyWithType(params, inner) = wt;
            let (elems, _) = match inner.kind {
                TyKind::Tuple(t) => fold_splat_elems(t.0),
                _ => splat_expand(*inner),
            };
            (elems, Some(params))
        }
        // Anything else (primitive / generic / nested containers that belong
        // to the element itself, e.g. `Vec<().2>`) stays a single element.
        other => (vec![Ty { span: ty.span, kind: other }], None),
    }
}

fn fold_splat_elems(elems: Vec<Ty>) -> (Vec<Ty>, Option<TyTypeParam>) {
    elems.into_iter().fold((vec![], None), |(mut flat, decl), e| {
        let (mut es, d) = splat_expand(e);
        flat.append(&mut es);
        (flat, merge_decls(decl, d))
    })
}

/// Maps a generic parameter list through `f` — the parameter positions of
/// `TyTypeParam`: positional params (name + optional bound) and
/// associated-type bindings (name + value) are all `Ty`-bearing.
fn map_type_param(tp: TyTypeParam, f: &mut impl FnMut(Ty) -> Ty) -> TyTypeParam {
    let params =
        tp.params.into_iter().map(|(n, b)| (f(*n).into(), b.map(|b| f(*b).into()))).collect();
    let bindings =
        tp.bindings.into_iter().map(|(n, v)| (Box::new(f(*n)), Box::new(f(*v)))).collect();
    TyTypeParam { params, bindings }
}

/// Flatten top-level splat params (`T<*(A,B)>` → `T<A,B>`) and hoist
/// generator declarations (`T<().2>` = `<A,B>T<(A,B)>`) without recursing
/// into ordinary names; returns flat params + any hoisted declaration.
/// Shared by `expand_tp` (structure level, recurses afterwards) and
/// `extract_impl_parts` (trait args, rendered to tokens).
pub(crate) fn flat_splat_params(params: TyParams) -> (TyParams, Option<TyTypeParam>) {
    let mut flat = vec![];
    let mut decl = None;
    for (name, bound) in params {
        match name.kind {
            // `*(A,B)` param → its flat elements
            TyKind::Splat(_) => {
                let (es, d) = splat_expand(*name);
                decl = merge_decls(decl, d);
                flat.extend(es.into_iter().map(|e| (e.into(), None)));
            }
            // generator param (`().N`) → hoist the fresh declaration; the
            // inner tuple stays the arg (`T<().2>` = `<A,B>T<(A,B)>`), but a
            // splat re-wrap (`*().N` → `<A,B>T<A,B>`) flattens further.
            TyKind::WithType(wt) => {
                decl = merge_decls(decl, Some(wt.0));
                let inner = *wt.1;
                match inner.kind {
                    TyKind::Splat(_) => {
                        let (es, d) = splat_expand(inner);
                        decl = merge_decls(decl, d);
                        flat.extend(es.into_iter().map(|e| (e.into(), None)));
                    }
                    _ => flat.push((inner.into(), bound)),
                }
            }
            _ => flat.push((name, bound)),
        }
    }
    (flat, decl)
}

/// Merge two optional fresh declarations (`TyTypeParam::extend` semantics).
pub(crate) fn merge_decls(a: Option<TyTypeParam>, b: Option<TyTypeParam>) -> Option<TyTypeParam> {
    match (a, b) {
        (None, b) => b,
        (a, None) => a,
        (Some(mut a), Some(b)) => {
            a.extend(b);
            Some(a)
        }
    }
}

/// Shared "recurse inner and rewrap" logic for wrapper variants: `make` rebuilds
/// the wrapper from the inner; when `inner` is `None` (bare wrapper), `make(None)`
/// returns it as-is (a leaf). Reused by the WithCode/WithWhere/WithAttr/WithPrefix arms.
pub(super) fn expand_wrapped<F>(make: F, inner: Option<Box<Ty>>) -> Expand
where
    F: Fn(Option<Box<Ty>>) -> Ty,
{
    match inner {
        Some(i) => match i.expand() {
            Expand::Many(v) => Expand::Many(v.into_iter().map(|e| make(e.into())).collect()),
            Expand::Leaf(l) => Expand::Leaf(make(l.into())),
        },
        None => Expand::Leaf(make(None)),
    }
}

/// Like [`expand_wrapped`], but the inner always exists (`WithType`/`WithTrait`
/// boxes are non-`Option`).
pub(super) fn expand_rebuild<F>(make: F, inner: Ty) -> Expand
where
    F: Fn(Box<Ty>) -> Ty,
{
    match inner.expand() {
        Expand::Many(v) => Expand::Many(v.into_iter().map(|e| make(e.into())).collect()),
        Expand::Leaf(l) => Expand::Leaf(make(l.into())),
    }
}

impl Ty {
    /// Maps every child `Ty` node — **including the parameter positions**:
    /// generic argument lists (`T<...>` params + bounds + associated-type
    /// bindings), generic declarations (`<...>` on `WithType`), and trait
    /// argument lists (`WithTrait`) are children too. Rebuilds the node with
    /// its span preserved. Single exhaustive home for the "recurse into
    /// children" pattern — `hoist_type_params`, error collection and future
    /// rebuild-style traversals compose on top of it instead of re-matching
    /// every `TyKind` variant.
    #[allow(clippy::redundant_closure)] // `&mut FnMut` cannot be moved into `.map(f)`
    pub(crate) fn map_children(self, f: &mut impl FnMut(Ty) -> Ty) -> Ty {
        let span = self.span;
        match self.kind {
            TyKind::Array(a) => {
                TyArray(a.0.into_iter().map(|e| f(e)).collect()).to_ty().with_span(span)
            }
            TyKind::Tuple(t) => {
                TyTuple(t.0.into_iter().map(|e| f(e)).collect()).to_ty().with_span(span)
            }
            TyKind::Group(g) => TyGroup(f(*g.0).into()).to_ty().with_span(span),
            TyKind::PrimitiveArray(pa) => {
                TyPrimitiveArray(pa.0.map(|e| f(*e).into()), pa.1).to_ty().with_span(span)
            }
            TyKind::Generic(g) => {
                TyGeneric(f(*g.0).into(), map_type_param(g.1, f)).to_ty().with_span(span)
            }
            TyKind::WithPrefix(wp) => {
                TyWithPrefix(wp.0, wp.1.map(|e| f(*e).into())).to_ty().with_span(span)
            }
            TyKind::WithDyn(wd) => TyWithDyn(Box::new(f(*wd.0)), wd.1).to_ty().with_span(span),
            TyKind::WithFor(wf) => TyWithFor(wf.0, Box::new(f(*wf.1))).to_ty().with_span(span),
            TyKind::WithTrait(wt) => {
                TyWithTrait(TyTrait(wt.0.0, map_type_param(wt.0.1, f)), f(*wt.1).into())
                    .to_ty()
                    .with_span(span)
            }
            TyKind::WithCode(wc) => {
                TyWithCode(wc.0.map(|e| f(*e).into()), wc.1).to_ty().with_span(span)
            }
            TyKind::WithWhere(ww) => {
                TyWithWhere(ww.0.map(|e| f(*e).into()), ww.1).to_ty().with_span(span)
            }
            TyKind::WithImpl(wi) => {
                TyWithImpl(wi.0.map(|e| f(*e).into()), wi.1).to_ty().with_span(span)
            }
            TyKind::WithType(wt) => {
                TyWithType(map_type_param(wt.0, f), f(*wt.1).into()).to_ty().with_span(span)
            }
            TyKind::WithAttr(wa) => {
                TyWithAttr(wa.0, wa.1.map(|e| f(*e).into())).to_ty().with_span(span)
            }
            TyKind::Fn(fn_) => TyFn(
                fn_.0.map(|params| params.into_iter().map(|p| f(p)).collect()),
                fn_.1.map(|r| f(*r).into()),
                fn_.2,
                fn_.3,
            )
            .to_ty()
            .with_span(span),
            // `+` bound lists recurse element-wise like tuples.
            TyKind::BoundList(b) => {
                TyBoundList(b.0.into_iter().map(|e| f(e)).collect()).to_ty().with_span(span)
            }
            // No children: keep as-is.
            other => Ty { span, kind: other },
        }
    }
}
