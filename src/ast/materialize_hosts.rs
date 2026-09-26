//! Reconstruction of ordinary type hosts after their slots are collected.
//! Generic arguments are rebuilt directly, never replayed as application.

use proc_macro2::Span;

use super::materialize::{Branch, Materializer, Result, Rows, check_size, max_mass};
use super::*;

impl Materializer {
    fn singles(&mut self, values: Vec<Ty>, depth: usize, span: Span) -> Result<Rows> {
        let mut rows = vec![Branch::bare(vec![], 0)];
        for value in values {
            let next = self.one(value, depth + 1)?;
            rows = self.product(rows, next, span, |mut items, item| {
                items.push(item);
                items
            })?;
        }
        Ok(rows)
    }

    fn optional(
        &mut self, inner: Option<Box<Ty>>, depth: usize, span: Span,
        wrap: impl Fn(Option<Box<Ty>>) -> Ty,
    ) -> Result<Rows> {
        let Some(inner) = inner else {
            return Ok(vec![Branch::bare(vec![wrap(None)], 1)]);
        };
        wrap_slots(self.slots(*inner, depth + 1)?, span, |item| wrap(Some(item.into())))
    }

    pub(super) fn host(&mut self, value: Ty, depth: usize) -> Result<Rows> {
        let Ty { span, kind } = value;
        match kind {
            TyKind::Tuple(tuple) => self
                .combine(tuple.0, depth, span)?
                .into_iter()
                .map(|row| Ok(row.map(|items| vec![TyTuple(items).to_ty().with_span(span)], 1)))
                .collect(),
            TyKind::Generic(generic) => {
                let base = self.one(*generic.0, depth + 1)?;
                let params = self.params(generic.1, false, depth + 1, span)?;
                let rows =
                    self.product(base, params, span, |base, args| TyGeneric(base.into(), args))?;
                Ok(rows
                    .into_iter()
                    .map(|row| row.map(|ty| vec![ty.to_ty().with_span(span)], 1))
                    .collect())
            }
            TyKind::Trait(path) => {
                let params = self.params(path.1, false, depth + 1, span)?;
                Ok(params
                    .into_iter()
                    .map(|row| {
                        row.map(
                            |args| vec![TyTrait(path.0.clone(), args).to_ty().with_span(span)],
                            1,
                        )
                    })
                    .collect())
            }
            TyKind::TypeParam(params) => Ok(self
                .params(params, true, depth + 1, span)?
                .into_iter()
                .map(|row| row.map(|args| vec![args.to_ty().with_span(span)], 1))
                .collect()),
            TyKind::WithTrait(trait_) => {
                let args = self.params(trait_.0.1, false, depth + 1, span)?;
                let body = self.slots(*trait_.1, depth + 1)?;
                let mut out = vec![];
                for row in self.product(args, body, span, |args, body| (args, body))? {
                    let (args, body) = row.value;
                    let args_mass = count_leaves(&args.clone().to_ty()).saturating_sub(1);
                    let mass = row
                        .mass
                        .saturating_sub(args_mass)
                        .saturating_add(body.len().saturating_mul(args_mass.saturating_add(1)));
                    check_size(out.len() + 1, max_mass(&out).max(mass), span)?;
                    let value = body
                        .into_iter()
                        .map(|item| {
                            TyWithTrait(TyTrait(trait_.0.0.clone(), args.clone()), item.into())
                                .to_ty()
                                .with_span(span)
                        })
                        .collect();
                    out.push(Branch { value, decl: row.decl, mass });
                }
                Ok(out)
            }
            TyKind::PrimitiveArray(array) => match array.0 {
                None => Ok(vec![Branch::bare(
                    vec![TyPrimitiveArray(None, array.1).to_ty().with_span(span)],
                    1,
                )]),
                Some(inner) => Ok(self
                    .one(*inner, depth + 1)?
                    .into_iter()
                    .map(|row| {
                        row.map(
                            |ty| {
                                vec![
                                    TyPrimitiveArray(ty.into(), array.1.clone())
                                        .to_ty()
                                        .with_span(span),
                                ]
                            },
                            1,
                        )
                    })
                    .collect()),
            },
            TyKind::WithPrefix(prefix) => match prefix.0 {
                TyPrefix::Unsafe => self.optional(prefix.1, depth, span, |inner| {
                    TyWithPrefix(prefix.0, inner).to_ty().with_span(span)
                }),
                _ => match prefix.1 {
                    None => Ok(vec![Branch::bare(
                        vec![TyWithPrefix(prefix.0, None).to_ty().with_span(span)],
                        1,
                    )]),
                    Some(inner) => Ok(self
                        .one(*inner, depth + 1)?
                        .into_iter()
                        .map(|row| {
                            row.map(
                                |ty| {
                                    vec![TyWithPrefix(prefix.0, ty.into()).to_ty().with_span(span)]
                                },
                                1,
                            )
                        })
                        .collect()),
                },
            },
            TyKind::Prefixed(prefix) => Ok(self
                .one(*prefix.1, depth + 1)?
                .into_iter()
                .map(|row| {
                    row.map(
                        |ty| vec![TyPrefixed(prefix.0.clone(), ty.into()).to_ty().with_span(span)],
                        1,
                    )
                })
                .collect()),
            TyKind::Fn(callable) => {
                let params = match callable.0 {
                    Some(params) => self
                        .combine(params, depth, span)?
                        .into_iter()
                        .map(|row| row.map(Some, 0))
                        .collect(),
                    None => vec![Branch::bare(None, 0)],
                };
                let returns = match callable.1 {
                    Some(ret) => self
                        .one(*ret, depth + 1)?
                        .into_iter()
                        .map(|row| row.map(|ty| Some(ty.into()), 0))
                        .collect(),
                    None => vec![Branch::bare(None, 0)],
                };
                Ok(self
                    .product(params, returns, span, |params, ret| {
                        TyFn(params, ret, callable.2, callable.3)
                    })?
                    .into_iter()
                    .map(|row| row.map(|ty| vec![ty.to_ty().with_span(span)], 1))
                    .collect())
            }
            TyKind::BoundList(bounds) => Ok(self
                .singles(bounds.0, depth, span)?
                .into_iter()
                .map(|row| row.map(|items| vec![TyBoundList(items).to_ty().with_span(span)], 1))
                .collect()),
            TyKind::WithDyn(dynamic) => {
                let inner = self.one(*dynamic.0, depth + 1)?;
                let bounds = self.singles(dynamic.1.0, depth, span)?;
                Ok(self
                    .product(inner, bounds, span, |inner, bounds| {
                        TyWithDyn(inner.into(), TyBoundList(bounds))
                    })?
                    .into_iter()
                    .map(|row| row.map(|ty| vec![ty.to_ty().with_span(span)], 1))
                    .collect())
            }
            TyKind::WithFor(bound) => {
                let binder = self.singles(bound.0, depth, span)?;
                let inner = self.one(*bound.1, depth + 1)?;
                Ok(self
                    .product(binder, inner, span, |binder, inner| TyWithFor(binder, inner.into()))?
                    .into_iter()
                    .map(|row| row.map(|ty| vec![ty.to_ty().with_span(span)], 1))
                    .collect())
            }
            TyKind::Qualified(qualified) => {
                let (inner, trait_) = match qualified.0 {
                    QualifiedHead::Type(inner) => (inner, None),
                    QualifiedHead::Projection(inner, trait_) => (inner, Some(trait_)),
                };
                Ok(self
                    .one(*inner, depth + 1)?
                    .into_iter()
                    .map(|row| {
                        row.map(
                            |inner| {
                                let head = match &trait_ {
                                    Some(trait_) => {
                                        QualifiedHead::Projection(inner.into(), trait_.clone())
                                    }
                                    None => QualifiedHead::Type(inner.into()),
                                };
                                vec![TyQualified(head, qualified.1.clone()).to_ty().with_span(span)]
                            },
                            1,
                        )
                    })
                    .collect())
            }
            TyKind::WithCode(w) => self.optional(w.0, depth, span, |inner| {
                TyWithCode(inner, w.1.clone()).to_ty().with_span(span)
            }),
            TyKind::WithWhere(w) => self.optional(w.0, depth, span, |inner| {
                TyWithWhere(inner, w.1.clone()).to_ty().with_span(span)
            }),
            TyKind::WithImpl(w) => self.optional(w.0, depth, span, |inner| {
                TyWithImpl(inner, w.1.clone()).to_ty().with_span(span)
            }),
            TyKind::WithAttr(w) => self.optional(w.1, depth, span, |inner| {
                TyWithAttr(w.0.clone(), inner).to_ty().with_span(span)
            }),
            kind @ (TyKind::Primitive(_)
            | TyKind::Num(_)
            | TyKind::Range(_)
            | TyKind::Fresh(_)
            | TyKind::Lifetime(_)) => Ok(vec![Branch::bare(vec![Ty { span, kind }], 1)]),
            // Structural dispatch belongs to slots, including its error path.
            kind @ (TyKind::Array(_)
            | TyKind::Pack(_)
            | TyKind::Group(_)
            | TyKind::WithType(_)
            | TyKind::Error(_)) => self.slots(Ty { span, kind }, depth + 1),
        }
    }
}

fn wrap_slots(rows: Rows, span: Span, wrap: impl Fn(Ty) -> Ty) -> Result<Rows> {
    let mass = rows.iter().map(|row| row.mass.saturating_add(row.value.len())).max().unwrap_or(0);
    check_size(rows.len(), mass, span)?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let extra = row.value.len();
            row.map(|items| items.into_iter().map(&wrap).collect(), extra)
        })
        .collect())
}
