//! The impl-generic concern: same-name declaration merging, trait-bound
//! inheritance, and impl-name normalization (kept as a focused concern file
//! in the codegen layer — order of application is described in `mod.rs`).

use std::collections::{HashMap, HashSet};

use proc_macro2::{TokenStream, TokenTree};
use quote::quote;

use crate::TraitBounds;
use crate::ast::{Ty, TyPrimitive};
use crate::codegen::extract::ImplParts;
use crate::util::compile_err;

/// Inherits trait generic bounds onto impl generic params **without a written
/// bound** (same-name inheritance, positional match) and appends the trait's
/// unmerged where predicates to the impl (after a reference check). Returns
/// the collected errors; on any error the caller emits only the errors — no
/// partial impl. Rules: see the `TraitBounds` docs.
pub(crate) fn inherit_trait_bounds(
    parts: &mut ImplParts, trait_bounds: &TraitBounds, trait_args: &[String],
    impl_names: &HashSet<String>,
) -> Vec<TokenStream> {
    let mut errs = vec![];
    for (name, bound) in &mut parts.impl_generics {
        if bound.is_some() {
            continue;
        }
        let key = name.to_string();
        // where this param appears as a trait argument (absent = trait-unrelated, no inherit)
        let Some(pos) = trait_args.iter().position(|a| a == &key) else {
            continue;
        };
        let Some(tp) = trait_bounds.params.get(pos) else {
            continue;
        };
        let Some(b) = &tp.bound else {
            continue;
        };
        if tp.name != key {
            errs.push(compile_err!(
                "batch-impl: trait argument `{}` maps to parameter `{}` (bound `{}`); automatic \
                 inheritance requires the same name; rename to `{}` or write the bound manually",
                key,
                tp.name,
                b,
                tp.name
            ));
            continue;
        }
        if let Some(r) = tp.refs.iter().find(|r| !impl_names.contains(*r)) {
            errs.push(compile_err!(
                "batch-impl: inherited bound `{}` references parameter `{}`, but the impl declares \
                 no such name; declare `{}` or write the bound manually",
                b,
                r,
                r
            ));
            continue;
        }
        *bound = Some(TyPrimitive(b.clone()).to_ty());
    }
    // unmerged where predicates (compound / lifetime): after ref-check, append to the impl where
    for (pred, refs) in &trait_bounds.extra_predicates {
        if let Some(r) = refs.iter().find(|r| !impl_names.contains(*r)) {
            errs.push(compile_err!(
                "batch-impl: inherited where predicate `{}` references parameter `{}`, \
                 but the impl declares no such name; declare `{}` or hand-write the where clause",
                pred,
                r,
                r
            ));
            continue;
        }
        parts.where_clauses.push(pred.clone());
    }
    errs
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
    let mut counts: HashMap<String, usize> = HashMap::new();
    for (name, _) in &parts.impl_generics {
        *counts.entry(bare_param_name(name).to_string()).or_insert(0) += 1;
    }
    let mut merged: Vec<(TokenStream, Option<Ty>)> = Vec::new();
    let mut extra_where: Vec<TokenStream> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
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
