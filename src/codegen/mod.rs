//! Codegen layer: impl block generation.
//!
//! Recursively dismantles each flattened leaf [`Ty`] (see `lib::parse_batch_trait_entry`)
//! into an [`ImplParts`] (impl generics, trait generics, associated type bindings,
//! target type, body, attrs, unsafe flag), then renders the final
//! `impl<...> Trait<...> for Target { ... }` block.

mod fresh;
mod impl_parts;
mod postprocess;
mod top_level;
mod where_at;

pub(crate) use fresh::*;
pub(crate) use impl_parts::*;
pub(crate) use postprocess::*;
pub(crate) use top_level::*;
pub(crate) use where_at::*;

use crate::TraitBounds;
use crate::ast::types_render::render_param;
use crate::ast::*;
use crate::util::{compile_err, compile_error_str};
use proc_macro2::{Ident, TokenStream, TokenTree};
use quote::quote;
use std::collections::HashSet;

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
    trait_bounds: &TraitBounds, trait_param_names: &[Ident],
) -> TokenStream {
    // bare code block: `{...}` as the whole spec → emit verbatim as a top-level item
    // (not wrapped in an impl block). A `!`-marked block (top-level macro form)
    // without an attached type has no spec body to prepend — error instead of
    // emitting invalid Rust (`!` is not an item).
    if let Ty { kind: TyKind::WithCode(TyWithCode(None, code)), .. } = &ty {
        let is_top_marked = matches!(
            code.0.clone().into_iter().next(),
            Some(TokenTree::Punct(p)) if p.as_char() == '!'
        );
        if is_top_marked {
            return compile_error_str(
                "batch-impl: a top-level `{! ...}` block needs an attached type \
                 (the spec body is prepended to the macro input)",
                code.0
                    .clone()
                    .into_iter()
                    .next()
                    .map_or_else(proc_macro2::Span::call_site, |t| t.span()),
            );
        }
        return code.0.clone();
    }
    // Top-level macro form: a chain ending in a `{! ...}` block (or the
    // `#cmd(args){body}` open-extension product) marks a macro call for
    // top-level emission — the `!` is stripped, the spec body (target type
    // + preceding blocks, merged in chain order into one Brace group) is
    // prepended to the macro input, and the call is emitted at top level
    // (no impl generated). The `{!}` block must be the last block.
    if let Some(result) = top_level_macro(&ty) {
        return match result {
            Ok((spec, mac)) => {
                if spec.is_empty() {
                    compile_error_str(
                        "batch-impl: a top-level `{! ...}` block needs an attached type \
                         (the spec body is prepended to the macro input)",
                        proc_macro2::Span::call_site(),
                    )
                } else if mac.is_empty() {
                    compile_error_str(
                        "batch-impl: a `{! ...}` top-level block must contain a macro \
                         call (e.g. `{! my_macro!{...}}`)",
                        proc_macro2::Span::call_site(),
                    )
                } else {
                    sweep_fresh_names(rewrite_macro_input(mac, spec))
                }
            }
            Err(e) => e,
        };
    }
    if let Ty { kind: TyKind::Error(e), .. } = ty {
        return e.0;
    }
    let mut parts = extract_impl_parts(ty);

    // Codegen postprocess: substitute trait generic params in the body
    // (`From<bool>`: `value: T` → `value: bool` — the directive-copied
    // signature and user code block). ImplParts carries the arg names.
    substitute_trait_generics(&mut parts, trait_param_names);

    // Tuple-level splat expansion (Ty structure): `(A, *(B,C))` → `(A,B,C)`,
    // with fresh declarations from `*()^N` hoisted. Runs before hoisting so
    // the lifted decl feeds into the impl generics. (Generic-arg splats like
    // `T<*(A,B)>` are token-level, expanded at render — see `expand_splats`.)
    parts.target_type = expand_splat_elems(parts.target_type);

    // hoist nested `WithType` (fresh generics) out of the target type, preventing `<A>` leaks
    let mut nested_params = vec![];
    parts.target_type = hoist_type_params(parts.target_type, &mut nested_params);
    parts.impl_generics.extend(nested_params);
    // inherit trait generic bounds: same-name inheritance vs. mismatch errors; see trait_bounds docs
    let mut errs = vec![];
    let trait_args = parts
        .trait_generic_names
        .iter()
        .map(|n| n.to_string())
        .collect::<Vec<String>>();
    // const params are named `const N` in the parse layer (the keyword is needed to
    // render `const N: usize`); normalize to `N` to match trait args and where-predicate refs
    let impl_name_streams = parts
        .impl_generics
        .iter()
        .map(|(n, _)| {
            let s = n.to_string();
            let bare = s.strip_prefix("const ").unwrap_or(&s);
            bare.parse().unwrap()
        })
        .collect::<Vec<TokenStream>>();
    let impl_names =
        impl_name_streams.iter().map(|n| n.to_string()).collect::<HashSet<String>>();
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
    // where-predicate macro-meta replacement (`@N` → impl generic N)
    let mut where_resolved = vec![];
    for pred in &parts.where_clauses {
        match resolve_where_at(pred, &impl_name_streams) {
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

    // Impl-header splat expansion: `T<*(A,B)>` -> `T<A,B>` (the only place
    // `*()`/`*[]` flatten — parse/apply/expand keep them whole). Bodies are
    // never touched, so `a * b` inside a fn stays multiplication.
    let impl_head = expand_splats(quote!(
        #unsafe_kw impl #impl_gen #trait_name #trait_gen for #target #where_clause
    ));
    let rendered = quote! {
        #(#attrs)*
        #impl_head {
            #(#body_tokens)*
        }
    };
    sweep_fresh_names(rendered)
}
