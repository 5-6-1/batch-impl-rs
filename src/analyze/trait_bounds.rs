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
//! Reference collection happens on the **syn AST** ([`syn::visit`]): the
//! first segment of a path (`T` in `T` / `T::Item` — a projection subject)
//! and generic args (the `T` in `Vec<T>`) are param reference positions;
//! path segments after `::` (associated type names), binding names (the
//! `Item` in `dyn Trait<Item = T>`), and HRTB binders (the `'a` in
//! `for<'a>`) are naturally excluded — token-level scanning cannot tell
//! these apart, the AST can.

use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::quote;
use syn::ItemTrait;
use syn::visit::{self, Visit};

/// Trait param: name + merged bound (inline + where predicates) + param names referenced
/// by the bound.
#[derive(Default)]
pub(crate) struct TraitParam {
    pub(crate) name: String,
    pub(crate) bound: Option<TokenStream>,
    pub(crate) refs: Vec<String>,
}

/// List of the trait's generic params (positionally matching the trait args in a spec),
/// letting codegen inherit bounds for **impl generic params without written bounds** by
/// position + name.
///
/// Automation only recognizes matching names (`A<>` copying / `<T> A<T>` name-based
/// inheritance):
/// - an impl param maps to the param at its name's position in the trait args; same name
///   with a bound on the param → inherit;
/// - different name → `compile_error!` (rename it or hand-write the bound);
/// - inherited bounds/predicates referencing other param names (the `'a` in `T: 'a`, the `A`
///   in `A::B`) that the impl lacks under the same name → `compile_error!` (declare or
///   hand-write).
///
/// Writing bounds is the user's job, the macro does not interfere (the macro cannot infer
/// sub-trait entailment (`trait B: A` making `T: B` imply `T: A`)). Single-param predicates
/// of the trait-level where clause are merged into bounds; the rest pass through verbatim
/// ([`TraitBounds::extra_predicates`]).
#[derive(Default)]
pub(crate) struct TraitBounds {
    pub(crate) params: Vec<TraitParam>,
    /// Where predicates not merged into bounds (compound / lifetime predicates) plus the
    /// param names they reference. codegen appends them to the impl's where clause and runs
    /// a reference check (rename scenarios get guided errors).
    pub(crate) extra_predicates: Vec<(TokenStream, Vec<String>)>,
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
    // Set of param names (type + const are idents, lifetimes carry a `'` prefix)
    let type_const_names = trait_item
        .generics
        .params
        .iter()
        .filter_map(|p| match p {
            syn::GenericParam::Type(tp) => Some(tp.ident.to_string()),
            syn::GenericParam::Const(cp) => Some(cp.ident.to_string()),
            _ => None,
        })
        .collect::<Vec<String>>();
    let lt_names = trait_item
        .generics
        .params
        .iter()
        .filter_map(|p| match p {
            syn::GenericParam::Lifetime(ld) => Some(format!("'{}", ld.lifetime.ident)),
            _ => None,
        })
        .collect::<Vec<String>>();
    let mut params = vec![];
    for p in &trait_item.generics.params {
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
                let refs = collect_bound_refs(&tp.bounds, &type_const_names, &lt_names);
                params.push(TraitParam { name: tp.ident.to_string(), bound, refs });
            }
            syn::GenericParam::Lifetime(ld) => params.push(TraitParam {
                name: format!("'{}", ld.lifetime.ident),
                bound: None,
                refs: vec![],
            }),
            syn::GenericParam::Const(cp) => {
                params.push(TraitParam { name: cp.ident.to_string(), bound: None, refs: vec![] })
            }
        }
    }
    let mut extra_predicates = vec![];
    if let Some(wc) = &trait_item.generics.where_clause {
        for pred in &wc.predicates {
            let tokens = quote!(#pred);
            // Single-param predicate (`X: Bound`) merges into the bound at the matching position
            if let syn::WherePredicate::Type(pt) = pred
                && let Some(name) = single_ident_param(&pt.bounded_ty)
                && let Some(pos) = params.iter().position(|p| p.name == name)
            {
                // Note: quote interpolation only supports `#ident`, not field access
                let b = &pt.bounds;
                let extra = quote!(#b);
                let extra_refs = collect_bound_refs(&pt.bounds, &type_const_names, &lt_names);
                let param = &mut params[pos];
                param.bound = Some(match &param.bound {
                    Some(inline) => quote!(#inline + #extra),
                    None => extra,
                });
                param.refs.extend(extra_refs);
                continue;
            }
            // Remaining predicates: pass through verbatim + collect references
            let refs = collect_predicate_refs(pred, &type_const_names, &lt_names);
            extra_predicates.push((tokens, refs));
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

/// Collect the param names referenced by a bound list (`Clone + Send`, HRTB, etc.).
fn collect_bound_refs(
    bounds: &syn::punctuated::Punctuated<syn::TypeParamBound, syn::Token![+]>,
    type_const_names: &[String], lt_names: &[String],
) -> Vec<String> {
    let mut c = Collector::new(type_const_names, lt_names);
    for b in bounds {
        c.visit_type_param_bound(b);
    }
    c.refs
}

/// Collect the param names referenced by a where predicate (left type + bounds).
fn collect_predicate_refs(
    pred: &syn::WherePredicate, type_const_names: &[String], lt_names: &[String],
) -> Vec<String> {
    let mut c = Collector::new(type_const_names, lt_names);
    c.visit_where_predicate(pred);
    c.refs
}

/// syn AST reference collector.
///
/// Precise rules (AST semantics, not token guessing):
/// - Single-segment path (`T`) → the ident is the type name itself; colliding with a param
///   name means a reference; segments after `::` (associated type names) and generic args
///   (the `T` in `Vec<T>`) are handled by the default visit;
/// - HRTB binders (`for<'a>`) are pushed on a stack; the `'a` inside a binder is not collected;
/// - Associated type binding names (the `Item` in `dyn Trait<Item = T>`) are not types and
///   are not collected.
struct Collector<'a> {
    type_const_names: &'a [String],
    lt_names: &'a [String],
    /// HRTB binder stack (the `'a` in `for<'a>` is a local name that shadows a same-named
    /// outer param)
    hrtb: Vec<HashSet<String>>,
    refs: Vec<String>,
}

impl<'a> Collector<'a> {
    fn new(type_const_names: &'a [String], lt_names: &'a [String]) -> Self {
        Collector { type_const_names, lt_names, hrtb: vec![], refs: vec![] }
    }

    fn in_hrtb(&self, name: &str) -> bool {
        self.hrtb.iter().any(|s| s.contains(name))
    }

    /// Push the binder name set; returns whether it was pushed (so the caller can restore)
    fn push_hrtb(&mut self, lifetimes: Option<&syn::BoundLifetimes>) -> bool {
        if let Some(bl) = lifetimes {
            let set = bl
                .lifetimes
                .iter()
                .filter_map(|p| match p {
                    syn::GenericParam::Lifetime(ld) => Some(format!("'{}", ld.lifetime.ident)),
                    _ => None,
                })
                .collect();
            self.hrtb.push(set);
            true
        } else {
            false
        }
    }
}

impl<'ast> Visit<'ast> for Collector<'_> {
    fn visit_expr(&mut self, node: &'ast syn::Expr) {
        // const generic args / array lengths (the N in `[T; N]`, the N in `Foo<N>`):
        // a single-segment path expression is a const param reference position (type
        // params cannot appear in expressions)
        if let syn::Expr::Path(ep) = node
            && ep.qself.is_none()
            && let Some(seg) = ep.path.segments.first()
            && ep.path.segments.len() == 1
            && matches!(&seg.arguments, syn::PathArguments::None)
            && self.type_const_names.contains(&seg.ident.to_string())
        {
            self.refs.push(seg.ident.to_string());
        }
        visit::visit_expr(self, node);
    }

    fn visit_lifetime(&mut self, node: &'ast syn::Lifetime) {
        let name = format!("'{}", node.ident);
        if !self.in_hrtb(&name) && self.lt_names.contains(&name) {
            self.refs.push(name);
        }
    }

    fn visit_trait_bound(&mut self, node: &'ast syn::TraitBound) {
        // `for<'a> Fn(&'a u8)`: the 'a inside the binder is a local name, not collected
        let pushed = self.push_hrtb(node.lifetimes.as_ref());
        visit::visit_trait_bound(self, node);
        if pushed {
            self.hrtb.pop();
        }
    }

    fn visit_type_fn_ptr(&mut self, node: &'ast syn::TypeFnPtr) {
        // `fn<'a>(&'a u8)` type: the binder shadows the same way
        let pushed = self.push_hrtb(node.lifetimes.as_ref());
        visit::visit_type_fn_ptr(self, node);
        if pushed {
            self.hrtb.pop();
        }
    }

    fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
        // Single-segment path (`T`) or the FIRST segment of a longer path
        // (`T::Item` — a projection subject; `T::Assoc` inside a bound): the
        // leading ident is the type itself, so colliding with a param name
        // means a reference. Segments after `::` (associated type names) are
        // never collected; generic args (the `T` in `Vec<T>`) are visited by
        // the default recursion.
        if node.qself.is_none()
            && let Some(seg) = node.path.segments.first()
            && matches!(&seg.arguments, syn::PathArguments::None)
            && self.type_const_names.contains(&seg.ident.to_string())
        {
            self.refs.push(seg.ident.to_string());
        }
        // Continue by default: generic args (the T in `Vec<T>`), qself (the T in
        // `<T as Trait>::Item`)
        visit::visit_type_path(self, node);
    }
}
