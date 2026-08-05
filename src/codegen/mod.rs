//! Codegen layer: impl block generation.
//!
//! Recursively dismantles each flattened leaf [`Ty`] (see `lib::parse_batch_trait_entry`)
//! into an [`ImplParts`] (impl generics, trait generics, associated type bindings,
//! target type, body, attrs, unsafe flag), then renders the final
//! `impl<...> Trait<...> for Target { ... }` block.

mod impl_parts;
pub(crate) use impl_parts::*;

use crate::TraitBounds;
use crate::ast::types_render::render_param;
use crate::ast::*;
use crate::util::{compile_err, compile_error_str};
use proc_macro2::{TokenStream, TokenTree};
use quote::quote;

/// Generates one impl block (for a single flattened leaf `Ty`).
///
/// `trait_bounds`: the trait's generic param list (positionally matching the spec's
/// trait arguments). Impl generics **without a bound** inherit by position + same name
/// (`trait Foo<T: Clone>` + `<T> Foo<T>` → `impl<T: Clone>`); mismatched names or
/// bounds referencing undeclared params error out; user-bounded params are untouched
/// (the sub-trait macro cannot infer; writing a bound = user's responsibility).
///
/// Three exits:
/// - `Ty::Error` → output the `compile_error!` stream directly;
/// - bare code block `WithCode(None, ...)` (an open-instruction expansion product) →
///   injected verbatim as a top-level item, not wrapped in an impl;
/// - otherwise → dismantle metadata (`extract_impl_parts`) → hoist nested generics
///   (`hoist_type_params`) → build generics / trait generics / impl body → render `quote!`
pub(crate) fn generate_impl(
    ty: Ty, trait_name: &TokenStream, is_unsafe_trait: bool,
    trait_bounds: &TraitBounds,
) -> TokenStream {
    if let Ty::Error(e) = ty {
        return e.0;
    }
    // bare code block: `{...}` as the whole spec → emit verbatim as a top-level item
    // (not wrapped in an impl block)
    if let Ty::WithCode(TyWithCode(None, code)) = &ty {
        return code.0.clone();
    }
    let mut parts = extract_impl_parts(ty);

    // hoist nested `WithType` (fresh generics) out of the target type, preventing `<A>` leaks
    let mut nested_params = vec![];
    parts.target_type = hoist_type_params(parts.target_type, &mut nested_params);
    parts.impl_generics.extend(nested_params);

    // inherit trait generic bounds: same-name inheritance vs. mismatch errors; see trait_bounds docs
    let mut errs: Vec<TokenStream> = vec![];
    let trait_args: Vec<String> =
        parts.trait_generic_names.iter().map(|n| n.to_string()).collect();
    // const params are named `const N` in the parse layer (the keyword is needed to
    // render `const N: usize`); normalize to `N` to match trait args and where-predicate refs
    let impl_name_streams: Vec<TokenStream> = parts
        .impl_generics
        .iter()
        .map(|(n, _)| {
            let s = n.to_string();
            let bare = s.strip_prefix("const ").unwrap_or(&s);
            bare.parse().unwrap()
        })
        .collect();
    let impl_names: std::collections::HashSet<String> =
        impl_name_streams.iter().map(|n| n.to_string()).collect();
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
        *bound = Some(Ty::Primitive(TyPrimitive(b.clone())));
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
    // where-predicate macro-meta replacement (`@N` → impl generic N, `@trait` → trait name)
    let mut where_resolved: Vec<TokenStream> = vec![];
    for pred in &parts.where_clauses {
        match resolve_where_at(pred, &impl_name_streams, trait_name) {
            Ok(p) => where_resolved.push(p),
            Err(e) => errs.push(e),
        }
    }
    if !errs.is_empty() {
        return errs.into_iter().collect();
    }
    let parts = parts; // only where_resolved is used afterwards; parts is no longer mutated

    let is_unsafe = is_unsafe_trait || parts.is_unsafe_impl;
    let unsafe_kw = if is_unsafe { quote!(unsafe) } else { quote!() };

    // impl generic params (with bounds)
    let impl_gen = if parts.impl_generics.is_empty() {
        quote!()
    } else {
        let params = parts
            .impl_generics
            .iter()
            .map(|(name, bound)| render_param(name, bound.as_ref()))
            .collect::<Vec<_>>();
        quote!(<#(#params),*>)
    };

    // trait generic params (names only)
    let trait_gen = if parts.trait_generic_names.is_empty() {
        quote!()
    } else {
        let names = &parts.trait_generic_names;
        quote!(<#(#names),*>)
    };

    // target type
    let target = &parts.target_type;

    // impl body: associated types + user body
    let mut body_tokens = vec![];
    for (name, value) in &parts.associated_types {
        body_tokens.push(quote!(type #name = #value;));
    }
    if let Some(body) = &parts.body {
        body_tokens.push(body.clone());
    }

    // attributes
    let attrs = parts.attrs;

    // where clause: join predicates with commas; empty if no where (resolve_where_at already ran)
    let where_clause = if where_resolved.is_empty() {
        quote!()
    } else {
        let preds = &where_resolved;
        quote!(where #(#preds),*)
    };

    quote! {
        #(#attrs)*
        #unsafe_kw impl #impl_gen #trait_name #trait_gen for #target #where_clause {
            #(#body_tokens)*
        }
    }
}

/// Macro-meta position references in where predicates: `@N` → the N-th impl generic
/// name, `@trait` → the trait name. `@N` out of range or a non-position digit / other
/// token after `@` errors. Blanket-wrapped where is pre-resolved; only user where
/// predicates are handled here (tuple/normal specs — `()^2 where{@0: Clone}`, `<T> where{@0: X}`).
fn resolve_where_at(
    pred: &TokenStream, impl_names: &[TokenStream], trait_name: &TokenStream,
) -> Result<TokenStream, TokenStream> {
    let tokens: Vec<_> = pred.clone().into_iter().collect();
    let mut out = vec![];
    let mut i = 0;
    while i < tokens.len() {
        if let TokenTree::Punct(p) = &tokens[i]
            && p.as_char() == '@'
        {
            match tokens.get(i + 1) {
                Some(TokenTree::Literal(lit)) => {
                    let idx: usize = lit.to_string().parse().map_err(|_| {
                        compile_error_str("batch-impl: `@` in a where predicate must be followed by a position digit (e.g. `@0`)")
                    })?;
                    let Some(name) = impl_names.get(idx) else {
                        return Err(compile_err!(
                            "batch-impl: `@{}` out of range in a where predicate (impl has {} generics, indexed from 0)",
                            idx,
                            impl_names.len()
                        ));
                    };
                    out.extend(name.clone());
                    i += 2;
                }
                Some(TokenTree::Ident(id)) if id == "trait" => {
                    out.extend(trait_name.clone());
                    i += 2;
                }
                _ => {
                    return Err(compile_error_str(
                        "batch-impl: `@` in a where predicate must be a position digit (e.g. `@0`) or `@trait`",
                    ));
                }
            }
        } else {
            out.push(tokens[i].clone());
            i += 1;
        }
    }
    Ok(out.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze::extract_trait_bounds;
    use syn::parse_quote;

    /// `WhereArr<>` expansion: impl generics `[T, const N: usize]` (parse-layer name is
    /// `const N`; the keyword is needed to render), trait args `[T, N]`, predicate
    /// `[T; N]: Sized` referencing N — after normalization the check passes and the
    /// expansion has no compile_error (regression guard against IDE/stale false positives)
    #[test]
    fn const_param_where_predicate_no_error() {
        let trait_def: syn::ItemTrait = parse_quote!(
            trait WhereArr<T, const N: usize>
            where
                [T; N]: Sized,
            {
            }
        );
        let tb = extract_trait_bounds(&trait_def);
        let target: Ty = TyTuple(vec![]).into();
        let trait_ty = TyTrait(
            quote!(WhereArr),
            TyTypeParam {
                params: vec![(quote!(T), None), (quote!(N), None)],
                bindings: vec![],
            },
        );
        let wrapped = TyWithTrait(trait_ty, target.into());
        let impl_ty = TyWithType(
            TyTypeParam {
                params: vec![
                    (quote!(T), None),
                    (
                        quote!(const N),
                        Some(Ty::Primitive(TyPrimitive(quote!(usize)))),
                    ),
                ],
                bindings: vec![],
            },
            wrapped.into(),
        )
        .into();
        let out = generate_impl(impl_ty, &quote!(WhereArr), false, &tb).to_string();
        assert!(
            !out.contains("compile_error"),
            "expansion must not contain compile_error: {out}"
        );
        assert!(
            out.contains("where [T ; N] : Sized"),
            "missing where predicate: {out}"
        );
        assert!(
            out.contains("impl < T , const N : usize > WhereArr < T , N >"),
            "unexpected impl generics: {out}"
        );
    }
}
