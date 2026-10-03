//! Generic arguments and declaration names share storage, not slot semantics.

use proc_macro2::{Span, TokenStream};
use quote::ToTokens;

use super::materialize::{Branch, Materializer, Result, check_size, max_mass};
use super::*;

impl Materializer {
    pub(super) fn params(
        &mut self, params: TyTypeParam, declarations: bool, depth: usize, span: Span,
    ) -> Result<Vec<Branch<TyTypeParam>>> {
        let mut rows = vec![Branch::bare(TyTypeParam { params: vec![], bindings: vec![] }, 0)];
        for (name, bound) in params.params {
            let names = self.slots(*name, depth + 1)?;
            // An argument list cannot lose its argument: a pack that expands to no
            // members would leave `Vec<>` for rustc to reject with E0107. (An empty
            // `<>` block is the deliberate `X<>` sync spelling and never gets here —
            // it has no source entry to expand.)
            if !declarations {
                for row in &names {
                    if row.value.is_empty() {
                        return Err(crate::util::compile_error_str(
                            "batch-impl: this argument list requires at least one type, but the \
                             pack expands to none (`*[]` is a star over the empty list)",
                            span,
                        ));
                    }
                }
            }
            if declarations {
                for row in &names {
                    if row.decl.is_some() {
                        return Err(crate::util::compile_error_str(
                            "batch-impl: a fresh generator cannot be declared here — the `<>` \
                             block declares the impl's own parameters, so its freshs would be \
                             declared and never used; write the generator on the type instead \
                             (e.g. `T.*[].2`)",
                            span,
                        ));
                    }
                    for name in &row.value {
                        validate_name(name)?;
                    }
                }
            }
            let bounds = match bound {
                Some(bound) => {
                    self.one(*bound, depth + 1)?.into_iter().map(|row| row.map(Some, 0)).collect()
                }
                None => vec![Branch::bare(None, 0)],
            };
            let mut fragments = vec![];
            for row in self.product(names, bounds, span, |names, bound| (names, bound))? {
                let (names, bound) = row.value;
                // A name pack can declare several names carrying one bound.
                // Only the bound payload is copied; its fresh declarations
                // are shared by all those declarations in this branch.
                let bound_mass = bound.as_ref().map_or(0, count_leaves);
                let mass = row
                    .mass
                    .saturating_sub(bound_mass)
                    .saturating_add(names.len().saturating_mul(bound_mass));
                check_size(fragments.len() + 1, max_mass(&fragments).max(mass), span)?;
                let params = names
                    .into_iter()
                    .map(|name| (name.into(), bound.clone().map(Into::into)))
                    .collect();
                fragments.push(Branch {
                    value: TyTypeParam { params, bindings: vec![] },
                    decl: row.decl,
                    mass,
                });
            }
            check_size(fragments.len(), max_mass(&fragments), span)?;
            rows = self.product(rows, fragments, span, |mut a, b| {
                a.extend(b);
                a
            })?;
        }
        for (name, value) in params.bindings {
            let names = self.one(*name, depth + 1)?;
            let values = self.one(*value, depth + 1)?;
            let fragments = self.product(names, values, span, |name, value| TyTypeParam {
                params: vec![],
                bindings: vec![(name.into(), value.into())],
            })?;
            rows = self.product(rows, fragments, span, |mut a, b| {
                a.extend(b);
                a
            })?;
        }
        Ok(rows)
    }
}

fn validate_name(name: &Ty) -> Result<()> {
    // Declaration-range references (`<@0..>`) remain part of the existing
    // late fresh protocol; their arity is known only after leaf selection.
    if matches!(&name.kind, TyKind::Fresh(_)) {
        return Ok(());
    }
    let tokens = name.to_token_stream();
    let valid = match ParamKind::of_name(&tokens) {
        ParamKind::Lifetime => syn::parse2::<syn::Lifetime>(tokens).is_ok(),
        ParamKind::Const => syn::parse2::<syn::Ident>(ParamKind::bare_name(&tokens)).is_ok(),
        ParamKind::Type => syn::parse2::<syn::Ident>(tokens).is_ok(),
    };
    if valid {
        return Ok(());
    }
    Err(invalid_name(name.span))
}

fn invalid_name(span: Span) -> TokenStream {
    crate::util::compile_error_str(
        "batch-impl: a generic declaration requires a parameter name (`T`, `'a`, or `const N`), not a constructed type",
        span,
    )
}
