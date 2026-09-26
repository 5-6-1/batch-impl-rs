//! Final structural collection. Packs splice slots; ordinary type hosts stay
//! intact, and choices remain separate branches throughout collection.

use proc_macro2::{Ident, Span, TokenStream};
use quote::ToTokens;

use super::*;
use crate::apply::{check_expand_limit, check_expand_mass};
use crate::util::{MAX_NEST_DEPTH, depth_err};

/// A branch's declarations survive independently of its slots, including an
/// empty slot list. `mass` counts payload nodes and declaration children, but
/// not the eventual single outer WithType node.
#[derive(Clone)]
pub(super) struct Branch<T> {
    pub(super) value: T,
    pub(super) decl: Option<TyTypeParam>,
    pub(super) mass: usize,
}

impl<T> Branch<T> {
    pub(super) fn bare(value: T, mass: usize) -> Self {
        Self { value, decl: None, mass }
    }

    pub(super) fn map<U>(self, f: impl FnOnce(T) -> U, extra: usize) -> Branch<U> {
        Branch { value: f(self.value), decl: self.decl, mass: self.mass.saturating_add(extra) }
    }
}

pub(super) type Rows = Vec<Branch<Vec<Ty>>>;
pub(super) type Result<T> = std::result::Result<T, TokenStream>;

/// Converts a completed DSL expression to ordinary leaf types. No application,
/// generation, fresh-name assignment or declaration deduplication runs here.
pub(crate) fn materialize_targets(value: Ty) -> Result<Vec<Ty>> {
    let span = value.span;
    let mut materializer = Materializer { work: 0 };
    let rows = materializer.slots(value, 0)?;
    let count = rows.iter().map(|r| r.value.len()).fold(0usize, usize::saturating_add);
    // A bare pack's declarations must accompany every emitted target. Check
    // their copied cost before cloning any carrier into those targets.
    let mut max_mass = 0;
    let work = rows
        .iter()
        .map(|r| {
            let carrier = r.decl.as_ref().map_or(0, |d| count_leaves(&d.clone().to_ty()));
            for item in &r.value {
                max_mass = max_mass.max(count_leaves(item).saturating_add(carrier));
            }
            let item_mass = r.value.iter().map(count_leaves).fold(0usize, usize::saturating_add);
            item_mass.saturating_add(carrier.saturating_mul(r.value.len()))
        })
        .fold(0usize, usize::saturating_add);
    check_size(count, max_mass, span)?;
    materializer.charge(work, span)?;
    Ok(rows
        .into_iter()
        .flat_map(|row| {
            row.value.into_iter().map(move |item| match &row.decl {
                Some(decl) => TyWithType(decl.clone(), item.into()).to_ty().with_span(span),
                None => item,
            })
        })
        .collect())
}

pub(super) fn check_size(count: usize, mass: usize, span: Span) -> Result<()> {
    if let Some(error) = check_expand_limit("materialization", count)
        .or_else(|| check_expand_mass("materialization", mass))
    {
        return Err(error.with_span(span).to_token_stream());
    }
    Ok(())
}

pub(super) fn total_mass<T>(rows: &[Branch<T>]) -> usize {
    rows.iter().map(|row| row.mass).fold(0usize, usize::saturating_add)
}

pub(super) fn max_mass<T>(rows: &[Branch<T>]) -> usize {
    rows.iter().map(|row| row.mass).max().unwrap_or(0)
}

pub(super) struct Materializer {
    work: usize,
}

impl Materializer {
    fn enter(&mut self, span: Span, depth: usize) -> Result<()> {
        if depth > MAX_NEST_DEPTH {
            return Err(depth_err(&[Ident::new("pack", span).into()], " during materialization"));
        }
        self.charge(1, span)
    }

    fn charge(&mut self, amount: usize, span: Span) -> Result<()> {
        self.work = self.work.saturating_add(amount);
        if self.work > MAX_EXPAND.saturating_mul(MAX_NEST_DEPTH) {
            return Err(crate::util::compile_error_str(
                "batch-impl: materialization work limit exceeded; simplify the nested candidates",
                span,
            ));
        }
        Ok(())
    }

    /// Cartesian composition, preserving source order. Limits are checked on
    /// the product and total copied payload before either input is cloned.
    pub(super) fn product<A: Clone, B: Clone, C>(
        &mut self, left: Vec<Branch<A>>, right: Vec<Branch<B>>, span: Span,
        join: impl Fn(A, B) -> C,
    ) -> Result<Vec<Branch<C>>> {
        let count = left.len().saturating_mul(right.len());
        let work = total_mass(&left)
            .saturating_mul(right.len())
            .saturating_add(total_mass(&right).saturating_mul(left.len()));
        let mass = if count == 0 { 0 } else { max_mass(&left).saturating_add(max_mass(&right)) };
        check_size(count, mass, span)?;
        self.charge(count.saturating_add(work), span)?;
        let mut out = Vec::with_capacity(count);
        for a in &left {
            for b in &right {
                out.push(Branch {
                    value: join(a.value.clone(), b.value.clone()),
                    decl: merge_decls(a.decl.clone(), b.decl.clone()),
                    mass: a.mass.saturating_add(b.mass),
                });
            }
        }
        Ok(out)
    }

    pub(super) fn combine(&mut self, values: Vec<Ty>, depth: usize, span: Span) -> Result<Rows> {
        let mut rows = vec![Branch::bare(vec![], 0)];
        for value in values {
            let next = self.slots(value, depth + 1)?;
            rows = self.product(rows, next, span, |mut a, b| {
                a.extend(b);
                a
            })?;
        }
        Ok(rows)
    }

    pub(super) fn one(&mut self, value: Ty, depth: usize) -> Result<Vec<Branch<Ty>>> {
        let span = value.span;
        let rows = self.slots(value, depth + 1)?;
        let mut out = vec![];
        for row in rows {
            if row.value.len() != 1 {
                return Err(crate::util::compile_error_str(
                    &format!(
                        "batch-impl: this type position requires exactly one type; the pack expands to {} types",
                        row.value.len()
                    ),
                    span,
                ));
            }
            // Length was checked above; iteration consumes the single item
            // without an indexing assumption or an unreachable fallback.
            out.extend(row.value.into_iter().map(|value| Branch {
                value,
                decl: row.decl.clone(),
                mass: row.mass,
            }));
        }
        Ok(out)
    }

    pub(super) fn slots(&mut self, value: Ty, depth: usize) -> Result<Rows> {
        let Ty { span, kind } = value;
        self.enter(span, depth)?;
        let rows = match kind {
            TyKind::Error(error) => return Err(error.0),
            TyKind::Array(array) => {
                let mut rows = vec![];
                let mut mass = 0usize;
                for child in array.0 {
                    let more = self.slots(child, depth + 1)?;
                    mass = mass.max(max_mass(&more));
                    check_size(rows.len().saturating_add(more.len()), mass, span)?;
                    rows.extend(more);
                }
                rows
            }
            TyKind::Pack(pack) => self.combine(pack.0, depth, span)?,
            TyKind::Group(group) => self.slots(*group.0, depth + 1)?,
            TyKind::WithType(carrier) => {
                let params = self.params(carrier.0, true, depth + 1, span)?;
                let body = self.slots(*carrier.1, depth + 1)?;
                self.product(params, body, span, |p, body| (p, body))?
                    .into_iter()
                    .map(|row| {
                        let (params, body) = row.value;
                        Branch {
                            value: body,
                            decl: merge_decls(Some(params), row.decl),
                            mass: row.mass,
                        }
                    })
                    .collect()
            }
            kind => self.host(Ty { span, kind }, depth)?,
        };
        check_size(rows.len(), max_mass(&rows), span)?;
        Ok(rows)
    }
}
