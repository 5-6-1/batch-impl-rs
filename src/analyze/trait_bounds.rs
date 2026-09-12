//! Source of truth for trait generic bound inheritance: extract the param mapping from the
//! trait definition.
//!
//! Lets codegen inherit, by position + name, bounds for **impl generic params without
//! written bounds** (`trait Foo<T: Clone>` + `<T> Foo<T>` → `impl<T: Clone>`).
//!
//! Trait-level where clause handling:
//! - **Single-param predicates** (`trait Foo<T> where T: Clone`, left side is a bare param
//!   name) → merged into the bound at that position (inline + where joined); `A<>` copying
//!   carries them along;
//! - **Remaining predicates** (`T::Item: Clone`, `Vec<T>: ...`, lifetime predicates, etc.) →
//!   stored verbatim in [`TraitBounds::extra_predicates`], appended by codegen to the impl's
//!   where clause — all predicate shapes covered, none dropped.
//!
//! Each param also carries its [`ParamKind`] — classified from the `syn` **variant**, which
//! is exact. Codegen used to re-derive the kind from the name string
//! (`name.starts_with('\'')`), the shape-guessing family [`ParamKind`] now owns.

use proc_macro2::TokenStream;
use quote::quote;
use syn::ItemTrait;

use crate::ast::ParamKind;

/// Trait param: name + kind + merged bound (inline + where predicates).
pub(crate) struct TraitParam {
    pub(crate) name: String,
    pub(crate) kind: ParamKind,
    pub(crate) bound: Option<TokenStream>,
}

/// List of the trait's generic params (positionally matching the trait args in a spec),
/// letting codegen inherit bounds for **impl generic params without written bounds**.
///
/// Inheritance is **positional substitution**, not name equality: the trait's params pair
/// with the spec's rendered trait args (`trait Store<T, K>` + `Store<u32, Vec<T>>` →
/// `T := u32`, `K := Vec<T>`), every trait param name occurring in a bound or an extra
/// predicate is replaced by its arg, and the result is appended. There is **no macro-level
/// reference check** any more: an earlier `syn::visit` collector produced name lists that
/// nothing read (the R4 entry in the dev changelog records its removal). A name the
/// substitution leaves behind is an ordinary rustc error — E0412 for an undeclared type,
/// E0207 for an unconstrained impl param — which is the diagnostic the user already knows.
///
/// Writing bounds is the user's job, the macro does not interfere (the macro cannot infer
/// sub-trait entailment (`trait B: A` making `T: B` imply `T: A`)). Single-param predicates
/// of the trait-level where clause are merged into bounds; the rest pass through verbatim
/// ([`TraitBounds::extra_predicates`]).
#[derive(Default)]
pub(crate) struct TraitBounds {
    pub(crate) params: Vec<TraitParam>,
    /// Where predicates not merged into bounds (compound / lifetime predicates), appended
    /// verbatim to the impl's where clause.
    pub(crate) extra_predicates: Vec<TokenStream>,
}

/// Collect generic param names (Lifetime → `'a`, Type/Const → ident).
///
/// Reused by `A<>` arg copying (empty_generics.rs) and `#blanket` generic args
/// (blanket.rs) — the two line-by-line isomorphic implementations converge here.
/// Note: quote interpolation does not support field access (`#tp.ident` would treat
/// `.ident` as a literal), so take a reference before interpolating.
pub(crate) fn generic_param_names(generics: &syn::Generics) -> Vec<TokenStream> {
    generics
        .params
        .iter()
        .map(|p| match p {
            syn::GenericParam::Lifetime(ld) => quote!(#ld),
            syn::GenericParam::Type(tp) => {
                let id = &tp.ident;
                quote!(#id)
            }
            syn::GenericParam::Const(cp) => {
                let id = &cp.ident;
                quote!(#id)
            }
        })
        .collect()
}

pub(crate) fn extract_trait_bounds(trait_item: &ItemTrait) -> TraitBounds {
    let mut params = vec![];
    for p in &trait_item.generics.params {
        let kind = ParamKind::of_generic_param(p);
        match p {
            syn::GenericParam::Type(tp) => {
                let bound = if tp.bounds.is_empty() {
                    None
                } else {
                    // Note: (quote interpolation does not support field access, take a
                    // reference first)
                    let b = &tp.bounds;
                    Some(quote!(#b))
                };
                params.push(TraitParam { name: tp.ident.to_string(), kind, bound });
            }
            syn::GenericParam::Lifetime(ld) => params.push(TraitParam {
                name: format!("'{}", ld.lifetime.ident),
                kind,
                bound: None,
            }),
            syn::GenericParam::Const(cp) => {
                params.push(TraitParam { name: cp.ident.to_string(), kind, bound: None })
            }
        }
    }
    let mut extra_predicates = vec![];
    if let Some(wc) = &trait_item.generics.where_clause {
        for pred in &wc.predicates {
            // Single-param predicate (`X: Bound`) merges into the bound at the matching
            // position.
            if let syn::WherePredicate::Type(pt) = pred
                && let Some(name) = single_ident_param(&pt.bounded_ty)
                && let Some(pos) = params.iter().position(|p| p.name == name)
            {
                // Note: quote interpolation only supports `#ident`, not field access
                let b = &pt.bounds;
                let extra = quote!(#b);
                let Some(param) = params.get_mut(pos) else {
                    continue;
                };
                param.bound = Some(match &param.bound {
                    Some(inline) => quote!(#inline + #extra),
                    None => extra,
                });
                continue;
            }
            // Remaining predicates: passed through verbatim.
            extra_predicates.push(quote!(#pred));
        }
    }
    TraitBounds { params, extra_predicates }
}

/// Whether the predicate's left side is a single param name (`T`: no path, no generic
/// args); returns the name.
fn single_ident_param(ty: &syn::Type) -> Option<String> {
    let syn::Type::Path(tp) = ty else { return None };
    if tp.qself.is_some() {
        return None;
    }
    let seg = tp.path.segments.first()?;
    if tp.path.segments.len() == 1 && matches!(&seg.arguments, syn::PathArguments::None) {
        Some(seg.ident.to_string())
    } else {
        None
    }
}
