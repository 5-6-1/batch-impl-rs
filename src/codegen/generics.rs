//! The impl-generic concern: same-name declaration merging, trait-bound
//! inheritance, and impl-name normalization (kept as a focused concern file
//! in the codegen layer — order of application is described in `mod.rs`).

use std::collections::{HashMap, HashSet};

use proc_macro2::{TokenStream, TokenTree};
use quote::{ToTokens, quote};

use crate::TraitBounds;
use crate::ast::{Ty, TyPrimitive};
use crate::codegen::extract::ImplParts;

/// Inherits trait-level constraints onto the generated impl — by
/// **positional substitution**, not by name equality:
///
/// - the trait's generic params (lifetimes included) pair positionally with
///   the spec's rendered trait args (`trait Store<T, K>` +
///   `Store<u32, Vec<T>>` → `T := u32`, `K := Vec<T>`);
/// - a type param's inline bound lands on the impl generic its arg names —
///   or, when the arg is anything else (a renamed generic covered by the
///   same-name arm; a *concrete* type like `u32` that cannot carry an
///   inline declaration), becomes a where predicate `{arg}: {bound}`;
/// - the trait's compound where predicates substitute every param with its
///   positional arg (`HashMap<T, K>: Send` → `HashMap<u32, Vec<T>>: Send`)
///   and join the impl where clause — no name-equality requirement.
///
/// Substitution is **path-aware**: an ident reached through `::` is a path
/// segment (`A::B`'s `B` is an associated type), never a parameter.
pub(crate) fn inherit_trait_bounds(
    parts: &mut ImplParts, trait_bounds: &TraitBounds, trait_args: &[TokenStream],
) {
    // The positional map: trait param name → the spec's rendered arg. Full
    // positional zip — lifetimes participate (`'a` → `'b` when the spec
    // renames them); Rust orders lifetimes first, so alignment holds.
    let map = trait_bounds
        .params
        .iter()
        .zip(trait_args.iter())
        .map(|(tp, arg)| (tp.name.clone(), arg.clone()))
        .collect::<Vec<_>>();
    let substitute = |ts| crate::util::subst::replace_map(ts, &map);

    // Inline bounds: each type param's bound lands on the impl generic its
    // positional arg names; a non-generic arg degrades to a where predicate.
    for (tp, arg) in trait_bounds.params.iter().zip(trait_args.iter()) {
        let Some(b) = &tp.bound else { continue };
        if tp.name.starts_with('\'') {
            // a lifetime parameter itself takes no inline bound here
            // (`'a: 'b` outlives declarations are out of scope)
            continue;
        }
        let arg_ident = bare_param_name(arg).to_string();
        let arg_is_bare = arg.to_string() == arg_ident;
        let slot = parts
            .impl_generics
            .iter_mut()
            .find(|(n, _)| arg_is_bare && bare_param_name(n).to_string() == arg_ident);
        let substituted = substitute(b);
        match slot {
            Some((_, slot_bound)) if slot_bound.is_none() => {
                *slot_bound = Some(TyPrimitive(substituted).to_ty());
            }
            _ => {
                // Concrete / composite arg: the constraint cannot ride an
                // inline declaration — emit it as a plain predicate.
                parts.where_clauses.push(quote!(#arg : #substituted));
            }
        }
    }

    // Compound where predicates: substitute every trait param positionally
    // and append. (The old name-equality ref-check is gone — substitution
    // removes every trait param name, and any *other* undeclared ident is
    // rustc's E0412 to report.)
    for (pred, _refs) in &trait_bounds.extra_predicates {
        parts.where_clauses.push(substitute(pred));
    }
}

/// Renders an impl generic name with the `const` keyword stripped (the parse
/// layer keeps `const` so `const N: usize` renders correctly; the bare name is
/// used for trait-arg matching and where-predicate references). Names are
/// always a single ident or the `const` ident pair; the fallback arm keeps the
/// token stream as-is so this helper can never panic (defensive — unreachable
/// in practice, kept to uphold the no-panic promise).
pub(crate) fn bare_param_name(name: &TokenStream) -> TokenStream {
    let mut tokens = name.clone().into_iter();
    match (tokens.next(), tokens.next()) {
        (Some(TokenTree::Ident(id)), None) => quote!(#id),
        (Some(TokenTree::Ident(kw)), Some(TokenTree::Ident(id)))
            if kw == "const" && tokens.next().is_none() =>
        {
            quote!(#id)
        }
        _ => name.clone(),
    }
}

/// Merges same-name impl generic declarations from chained `<>` blocks.
///
/// `<T: Clone><T: Copy> X` would render `impl<T: Clone, T: Copy>` — a
/// duplicate `T` declaration (E0415). Duplicate names collapse into one
/// **bare** declaration and every bound of that name moves into a where
/// predicate (`impl<T> ... where T: Clone, T: Copy`); the duplicate names
/// themselves are dropped. Names declared once are untouched (`<T: Clone>`
/// stays `impl<T: Clone>`). Const params (`const N: usize`) keep their full
/// declaration (the type annotation lives in the name tokens — there is
/// nowhere else for it to go; the later duplicates are simply dropped).
pub(crate) fn merge_dup_params(parts: &mut ImplParts) {
    let mut counts = HashMap::new();
    for (name, _) in &parts.impl_generics {
        *counts.entry(bare_param_name(name).to_string()).or_insert(0usize) += 1;
    }
    let mut merged = Vec::new();
    let mut extra_where = Vec::new();
    let mut seen = HashSet::new();
    for (name, bound) in std::mem::take(&mut parts.impl_generics) {
        let name_str = name.to_string();
        let is_const = name_str.starts_with("const");
        let key = bare_param_name(&name).to_string();
        if counts.get(&key).copied().unwrap_or(0) > 1 {
            // duplicate name: bare single declaration (or the first full
            // const declaration), every bound moved into a where predicate
            if is_const {
                if !seen.insert(key) {
                    continue; // drop later const duplicates entirely
                }
                merged.push((name, bound));
            } else {
                if seen.insert(key.clone()) {
                    merged.push((name.clone(), None));
                }
                if let Some(b) = bound {
                    extra_where.push(quote!(#name: #b));
                }
            }
        } else {
            merged.push((name, bound));
        }
    }
    parts.impl_generics = merged;
    parts.where_clauses.extend(extra_where);
}

/// Hoists fresh generics out of impl-generic **bounds**: a bound generator
/// (`<T: Fn.().2>` → the Fn's params come from `().2`, whose fresh
/// declarations live inside the bound Ty as a `WithType`) must have its
/// declarations ride out to the impl generics, leaving the bound as the bare
/// inner type (`T: Fn(P0, P1)`). Without this the bound renders
/// `T: <P0,P1> Fn(P0,P1)` — a generic declaration inside a predicate, which
/// rustc rejects.
pub(crate) fn hoist_bound_fresh(impl_generics: &mut Vec<(TokenStream, Option<Ty>)>) {
    let mut hoisted = vec![];
    for (_, bound) in impl_generics.iter_mut() {
        if let Some(b) = bound {
            let (stripped, fresh) = strip_bound_fresh(b);
            *b = stripped;
            hoisted.extend(fresh);
        }
    }
    impl_generics.extend(hoisted);
}

/// Strips `WithType` wrappers from a bound Ty, returning the inner type and
/// the hoisted declarations (each `WithType.params` entry, in order).
fn strip_bound_fresh(ty: &Ty) -> (Ty, Vec<(TokenStream, Option<Ty>)>) {
    use crate::ast::{TyKind, TyWithDyn, TyWithFor};
    match &ty.kind {
        TyKind::WithType(wt) => {
            let params =
                wt.0.params
                    .iter()
                    .map(|(n, b)| (n.to_token_stream(), b.clone().map(|b| *b)))
                    .collect::<Vec<_>>();
            let (inner, more) = strip_bound_fresh(&wt.1);
            let mut fresh = params;
            fresh.extend(more);
            (inner, fresh)
        }
        TyKind::WithDyn(wd) => {
            let (inner, fresh) = strip_bound_fresh(&wd.0);
            (TyWithDyn(Box::new(inner), wd.1.clone()).to_ty().with_span(ty.span), fresh)
        }
        TyKind::WithFor(wf) => {
            let (inner, fresh) = strip_bound_fresh(&wf.1);
            (TyWithFor(wf.0.clone(), Box::new(inner)).to_ty().with_span(ty.span), fresh)
        }
        _ => (ty.clone(), vec![]),
    }
}
