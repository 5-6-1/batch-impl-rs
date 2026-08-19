//! `Ty` traversal: parallel expansion ([`Expand`]) and the single child-map
//! home ([`Ty::map_children`]). Split from `types.rs` so node definitions and
//! traversal stay under the per-file budget.

use crate::apply::expand_limit_err;
use crate::ast::types::{
    MAX_EXPAND, Ty, TyArray, TyFn, TyGeneric, TyGroup, TyKind, TyParams, TyPrimitiveArray, TyTuple,
    TyTypeParam, TyWithAttr, TyWithCode, TyWithImpl, TyWithPrefix, TyWithTrait, TyWithType,
    TyWithWhere,
};
use crate::util::cartesian;

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

/// Whether a parameter list contains a generator (a `WithType` fresh
/// declaration) anywhere — the generic-declaration position cannot carry one
/// (`<*().N>` would render the fresh tuple as a parameter name).
pub(crate) fn contains_generator(params: &TyTypeParam) -> bool {
    params.params.iter().any(|(n, _)| ty_contains_generator(n))
        || params.bindings.iter().any(|(n, v)| ty_contains_generator(n) || ty_contains_generator(v))
}

fn ty_contains_generator(ty: &Ty) -> bool {
    match &ty.kind {
        TyKind::WithType(_) => true,
        TyKind::Generic(g) => {
            ty_contains_generator(&g.0)
                || g.1.params.iter().any(|(n, b)| {
                    ty_contains_generator(n) || b.as_ref().is_some_and(ty_contains_generator)
                })
                || g.1
                    .bindings
                    .iter()
                    .any(|(n, v)| ty_contains_generator(n) || ty_contains_generator(v))
        }
        TyKind::Array(a) => a.0.iter().any(ty_contains_generator),
        TyKind::Tuple(t) => t.0.iter().any(ty_contains_generator),
        TyKind::Splat(s) => s.elems().iter().any(ty_contains_generator),
        TyKind::Group(g) => ty_contains_generator(&g.0),
        TyKind::WithPrefix(w) => w.1.iter().any(|i| ty_contains_generator(i)),
        TyKind::WithAttr(w) => w.1.iter().any(|i| ty_contains_generator(i)),
        _ => false,
    }
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
fn expand_wrapped<F>(make: F, inner: Option<Box<Ty>>) -> Expand
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
fn expand_rebuild<F>(make: F, inner: Ty) -> Expand
where
    F: Fn(Box<Ty>) -> Ty,
{
    match inner.expand() {
        Expand::Many(v) => Expand::Many(v.into_iter().map(|e| make(e.into())).collect()),
        Expand::Leaf(l) => Expand::Leaf(make(l.into())),
    }
}

impl Ty {
    /// Maps every child `Ty` node (recursing into lists/optionals), rebuilding
    /// the node with its span preserved. Single exhaustive home for the
    /// "recurse into children" pattern — `hoist_type_params` and future
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
            TyKind::Generic(g) => TyGeneric(f(*g.0).into(), g.1).to_ty().with_span(span),
            TyKind::WithPrefix(wp) => {
                TyWithPrefix(wp.0, wp.1.map(|e| f(*e).into())).to_ty().with_span(span)
            }
            TyKind::WithTrait(wt) => TyWithTrait(wt.0, f(*wt.1).into()).to_ty().with_span(span),
            TyKind::WithCode(wc) => {
                TyWithCode(wc.0.map(|e| f(*e).into()), wc.1).to_ty().with_span(span)
            }
            TyKind::WithWhere(ww) => {
                TyWithWhere(ww.0.map(|e| f(*e).into()), ww.1).to_ty().with_span(span)
            }
            TyKind::WithImpl(wi) => {
                TyWithImpl(wi.0.map(|e| f(*e).into()), wi.1).to_ty().with_span(span)
            }
            TyKind::WithType(wt) => TyWithType(wt.0, f(*wt.1).into()).to_ty().with_span(span),
            TyKind::WithAttr(wa) => {
                TyWithAttr(wa.0, wa.1.map(|e| f(*e).into())).to_ty().with_span(span)
            }
            TyKind::Fn(fn_) => TyFn(
                fn_.0.map(|params| params.into_iter().map(|p| f(p)).collect()),
                fn_.1.map(|r| f(*r).into()),
                fn_.2,
            )
            .to_ty()
            .with_span(span),
            // No children: keep as-is.
            other => Ty { span, kind: other },
        }
    }

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
                    let dims: Vec<Vec<Ty>> =
                        t.0.iter()
                            .map(|e| match &e.kind {
                                TyKind::Array(a) => a.0.clone(),
                                _ => vec![e.clone()],
                            })
                            .collect();
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
            TyKind::Group(g) => (*g.0).expand(),
            TyKind::Generic(g) => {
                // Array args distribute like a list — `T<[A,B]>` → `[T<A>, T<B>]`
                // (Cartesian across multiple arrays). This is the single
                // authority for array-arg distribution: literal `[A,B]`, the
                // `[u8,...]` from a `@u*` constant, and the `TyArray` produced
                // by splat powers (`*(*@u*).2` → `[*(u8,u8), ...]`) all reach
                // params as a `TyArray` and distribute here.
                if g.1.params.iter().any(|(n, _)| matches!(n.kind, TyKind::Array(_))) {
                    let dims: Vec<TyParams> =
                        g.1.params
                            .iter()
                            .map(|(name, bound)| match &name.kind {
                                TyKind::Array(a) => {
                                    a.0.iter().map(|e| (e.clone().into(), bound.clone())).collect()
                                }
                                _ => vec![(name.clone(), bound.clone())],
                            })
                            .collect();
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
