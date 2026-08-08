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
use proc_macro2::{Group, Ident, Punct, Spacing, TokenStream, TokenTree};
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
    trait_bounds: &TraitBounds,
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
    let impl_names: HashSet<String> =
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
        *bound = Some(Ty::new(
            proc_macro2::Span::call_site(),
            TyPrimitive(b.clone()).into(),
        ));
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

    let rendered = quote! {
        #(#attrs)*
        #unsafe_kw impl #impl_gen #trait_name #trait_gen for #target #where_clause {
            #(#body_tokens)*
        }
    };
    sweep_fresh_names(rendered)
}

/// Detects the top-level macro form: a `WithCode` chain ending in a
/// `{! ...}` block (`!` as the block's first token — the open-extension
/// product or a user-written `T {! m!{...}}`). Returns the spec body tokens
/// (target type + preceding blocks in chain order, rendered) and the macro
/// call after the `!`. A `{!}` block must be the last block and there can
/// be at most one.
fn top_level_macro(
    ty: &Ty,
) -> Option<Result<(TokenStream, TokenStream), TokenStream>> {
    let mut body = vec![];
    let mut top: Option<TokenStream> = None;
    match walk_top_level(ty, &mut body, &mut top) {
        Ok(()) => top.map(|mac| Ok((body.into_iter().collect(), mac))),
        Err(e) => Some(Err(e)),
    }
}

fn walk_top_level(
    ty: &Ty, body: &mut Vec<TokenTree>, top: &mut Option<TokenStream>,
) -> Result<(), TokenStream> {
    match &ty.kind {
        TyKind::WithCode(TyWithCode(inner, code)) => {
            let tokens: Vec<TokenTree> = code.0.clone().into_iter().collect();
            let is_top = matches!(tokens.first(), Some(TokenTree::Punct(p)) if p.as_char() == '!');
            if is_top {
                if top.is_some() {
                    return Err(compile_error_str(
                        "batch-impl: at most one top-level `{! ...}` block per spec",
                        tokens
                            .first()
                            .map_or_else(proc_macro2::Span::call_site, |t| t.span()),
                    ));
                }
                *top = Some(tokens.into_iter().skip(1).collect());
                // Still walk the inner chain: a `{!}` nested inside would be a
                // second top-level block (error above); a plain block after
                // the `{!}` is impossible (the `{!}` is the outermost).
                if let Some(inner) = inner {
                    walk_top_level(inner, body, top)?;
                }
            } else {
                // Plain block: legal when it is a *preceding* block (the
                // `{!}` sits further out, i.e. the chain tail) — `T {b} {! m!{...}}`.
                // Illegal only when a `{!}` was found *inside* this block
                // (the `{!}` would not be the last block) — `T {! m!{...}} {b}`.
                let top_before = top.is_some();
                if let Some(inner) = inner {
                    walk_top_level(inner, body, top)?;
                }
                if top.is_some() && !top_before {
                    return Err(compile_error_str(
                        "batch-impl: a `{! ...}` top-level block must be the last block",
                        tokens
                            .first()
                            .map_or_else(proc_macro2::Span::call_site, |t| t.span()),
                    ));
                }
                body.extend(code.0.clone());
            }
        }
        _ => body.extend(render_ty_tokens(ty)),
    }
    Ok(())
}

/// Renders a non-WithCode Ty to tokens (the spec body's target type part).
fn render_ty_tokens(ty: &Ty) -> Vec<TokenTree> {
    quote!(#ty).into_iter().collect()
}

/// Prepends the spec body (as a single Brace group) to the macro call's
/// input group: `name!{ (args){body} trait }` →
/// `name!{ {spec} (args){body} trait }` (the spec group goes *inside* the
/// macro input group, right after the opening delimiter).
fn rewrite_macro_input(mac: TokenStream, spec: TokenStream) -> TokenStream {
    let tokens: Vec<TokenTree> = mac.into_iter().collect();
    let mut out: Vec<TokenTree> = Vec::with_capacity(tokens.len() + 1);
    let mut inserted = false;
    let mut i = 0;
    while i < tokens.len() {
        if !inserted
            && matches!(&tokens[i], TokenTree::Punct(p) if p.as_char() == '!')
            && let Some(TokenTree::Group(g)) = tokens.get(i + 1)
        {
            out.push(tokens[i].clone()); // `!`
            // The spec body becomes the first *group* of the macro input
            // (`{spec}`) — the 4-segment protocol expects a Brace group.
            let spec_group = Group::new(proc_macro2::Delimiter::Brace, spec.clone());
            let mut inner = TokenStream::new();
            inner.extend(std::iter::once(TokenTree::Group(spec_group)));
            inner.extend(g.stream());
            let mut new_g = Group::new(g.delimiter(), inner);
            new_g.set_span(g.span());
            out.push(TokenTree::Group(new_g));
            inserted = true;
            i += 2;
            continue;
        }
        out.push(tokens[i].clone());
        i += 1;
    }
    out.into_iter().collect()
}

/// Sweeps grouped fresh names (`_Param_{g}_{i}_BatchGen_`) in a rendered
/// impl: renumbers them by (group, position) order to `_Param_0..N_BatchGen_`
/// (per impl — independent impls may reuse the same final names). The
/// grouping decouples generation (per-spec group ids) from the final `@N`
/// numbering: `@N` → `_Param_{N}_BatchGen_` is a pure construction and always
/// matches the swept name of the impl's N-th fresh in document order. Names
/// that do not match the grouped form pass through (user-written names or the
/// single-numbered `@N`-constructed ones). Returns the input unchanged when
/// no grouped fresh names exist.
fn sweep_fresh_names(tokens: TokenStream) -> TokenStream {
    let mut groups: Vec<(usize, usize)> = vec![];
    collect_grouped_fresh(&tokens, &mut groups);
    if groups.is_empty() {
        return tokens;
    }
    groups.sort_unstable();
    groups.dedup();
    let map: std::collections::HashMap<(usize, usize), usize> =
        groups.iter().enumerate().map(|(k, &gi)| (gi, k)).collect();
    replace_grouped_fresh(tokens, &map)
}

/// Parses `_Param_{g}_{i}_BatchGen_`; returns `None` for any other ident
/// (including the single-numbered `_Param_{n}_BatchGen_` form constructed
/// from `@N` references).
fn parse_grouped_fresh(s: &str) -> Option<(usize, usize)> {
    let rest = s.strip_prefix("_Param_")?.strip_suffix("_BatchGen_")?;
    let (g, i) = rest.split_once('_')?;
    Some((g.parse().ok()?, i.parse().ok()?))
}

fn collect_grouped_fresh(tokens: &TokenStream, out: &mut Vec<(usize, usize)>) {
    for tt in tokens.clone() {
        match tt {
            TokenTree::Ident(id) => {
                if let Some(gi) = parse_grouped_fresh(&id.to_string()) {
                    out.push(gi);
                }
            }
            TokenTree::Group(g) => {
                let inner = g.stream();
                collect_grouped_fresh(&inner, out);
            }
            _ => {}
        }
    }
}

fn replace_grouped_fresh(
    tokens: TokenStream, map: &std::collections::HashMap<(usize, usize), usize>,
) -> TokenStream {
    let mut out = vec![];
    for tt in tokens {
        match tt {
            TokenTree::Ident(id) => {
                let s = id.to_string();
                if let Some(&k) = parse_grouped_fresh(&s).and_then(|gi| map.get(&gi))
                {
                    let name = format!("_Param_{}_BatchGen_", k);
                    out.push(TokenTree::Ident(Ident::new(&name, id.span())));
                } else {
                    out.push(TokenTree::Ident(id));
                }
            }
            TokenTree::Group(g) => {
                let inner = g.stream();
                let mut new_g =
                    Group::new(g.delimiter(), replace_grouped_fresh(inner, map));
                new_g.set_span(g.span());
                out.push(TokenTree::Group(new_g));
            }
            other => out.push(other),
        }
    }
    out.into_iter().collect()
}

/// Macro-meta position references in where predicates: `@N` → the N-th fresh
/// generic in document order (grouped fresh names `_Param_{g}_{i}_BatchGen_`
/// sorted by (group, position), which is exactly the order the codegen
/// sweeper renumbers to `_Param_0..N_BatchGen_`) — user-written params are
/// addressed by their own names; `@N` exists exactly because fresh names are
/// unknowable. `@N` out of range or a non-position digit / other token after
/// `@` errors. `@trait` is resolved earlier (constant stage for batch_impl,
/// segment-level replacement for batch_trait!) and never reaches here.
/// Blanket-wrapped where is pre-resolved; only user where predicates are
/// handled here.
fn resolve_where_at(
    pred: &TokenStream, impl_names: &[TokenStream],
) -> Result<TokenStream, TokenStream> {
    // Fresh params sorted by (group, position) — the sweep order, so `@N`
    // matches the final `_Param_{N}_BatchGen_` the sweeper will emit.
    let mut fresh_sorted: Vec<&TokenStream> = impl_names
        .iter()
        .filter(|n| parse_grouped_fresh(&n.to_string()).is_some())
        .collect();
    fresh_sorted.sort_by_key(|n| parse_grouped_fresh(&n.to_string()).unwrap());
    let tokens: Vec<_> = pred.clone().into_iter().collect();
    let mut out = vec![];
    let mut i = 0;
    while i < tokens.len() {
        if let TokenTree::Punct(p) = &tokens[i]
            && p.as_char() == '@'
        {
            match tokens.get(i + 1) {
                Some(TokenTree::Ident(id)) if id == "all_fresh" => {
                    // `@all_fresh: Bound` → every fresh generic gets the
                    // predicate tail (`_Param_0_: Bound, _Param_1_: Bound,
                    // ...`) — comma-separated, subject-only.
                    if fresh_sorted.is_empty() {
                        return Err(compile_error_str(
                            "batch-impl: `@all_fresh` in a where predicate but this impl has no fresh generics",
                            tokens[i].span(),
                        ));
                    }
                    if fresh_sorted.len() > MAX_EXPAND {
                        return Err(compile_err!(
                            "batch-impl: `@all_fresh` expands to {} predicates (max {}); use `@N..M` for a subset",
                            fresh_sorted.len(),
                            MAX_EXPAND
                        ));
                    }
                    let tail: Vec<TokenTree> = tokens[i + 2..].to_vec();
                    let comma = TokenTree::Punct(Punct::new(',', Spacing::Alone));
                    for (k, &name) in fresh_sorted.iter().enumerate() {
                        if k > 0 {
                            out.push(comma.clone());
                        }
                        out.extend(name.clone());
                        out.extend(tail.iter().cloned());
                    }
                    i = tokens.len();
                    continue;
                }
                Some(TokenTree::Literal(lit)) => {
                    let s = lit.to_string();
                    // `@N..M` / `@N..=M`: a contiguous fresh range — each
                    // indexed fresh gets the predicate tail (comma-separated).
                    // Out of range or over MAX_EXPAND predicates errors.
                    if let Ok(start) = s.parse::<usize>()
                        && matches!(tokens.get(i + 2), Some(TokenTree::Punct(p)) if p.as_char() == '.')
                        && matches!(tokens.get(i + 3), Some(TokenTree::Punct(p)) if p.as_char() == '.')
                    {
                        let inclusive = matches!(tokens.get(i + 4), Some(TokenTree::Punct(p)) if p.as_char() == '=');
                        let end_idx = if inclusive { i + 5 } else { i + 4 };
                        let Some(TokenTree::Literal(end_lit)) = tokens.get(end_idx)
                        else {
                            return Err(compile_error_str(
                                "batch-impl: a `@N..M` range in a where predicate must end with a number (e.g. `@0..=2`)",
                                tokens[i].span(),
                            ));
                        };
                        let Ok(end) = end_lit.to_string().parse::<usize>() else {
                            return Err(compile_error_str(
                                "batch-impl: a `@N..M` range in a where predicate must end with a number (e.g. `@0..=2`)",
                                end_lit.span(),
                            ));
                        };
                        let count = if inclusive {
                            end.saturating_sub(start) + 1
                        } else {
                            end.saturating_sub(start)
                        };
                        if count == 0 {
                            return Err(compile_err!(
                                "batch-impl: `@{}..{}` is an empty range (start \
                                 not below end); no predicates will be generated",
                                start,
                                end
                            ));
                        }
                        if end >= fresh_sorted.len() || start > end {
                            return Err(compile_err!(
                                "batch-impl: `@{}..{}` out of range in a where \
                                 predicate (impl has {} fresh generics, numbered \
                                 from 0 in document order)",
                                start,
                                end,
                                fresh_sorted.len()
                            ));
                        }
                        if count > MAX_EXPAND {
                            return Err(compile_err!(
                                "batch-impl: `@{}..{}` expands to {} predicates (max {})",
                                start,
                                end,
                                count,
                                MAX_EXPAND
                            ));
                        }
                        let tail: Vec<TokenTree> = tokens[end_idx + 1..].to_vec();
                        let comma = TokenTree::Punct(Punct::new(',', Spacing::Alone));
                        for (offset, &name) in
                            fresh_sorted[start..start + count].iter().enumerate()
                        {
                            if offset > 0 {
                                out.push(comma.clone());
                            }
                            out.extend(name.clone());
                            out.extend(tail.iter().cloned());
                        }
                        i = tokens.len();
                        continue;
                    }
                    if let Ok(idx) = s.parse::<usize>() {
                        // Document-order index: `@N` resolves to the N-th fresh
                        // after (group, position) sorting — the same order the
                        // sweeper renumbers to `_Param_0..N_BatchGen_`.
                        let Some(&name) = fresh_sorted.get(idx) else {
                            return Err(compile_err!(
                                "batch-impl: `@{}` out of range in a where predicate \
                                 (impl has {} fresh generics, numbered from 0 in \
                                 document order; user-written params are addressed \
                                 by name)",
                                idx,
                                fresh_sorted.len()
                            ));
                        };
                        out.extend(name.clone());
                        i += 2;
                        continue;
                    }
                    // `@g_i` (literal with an underscore): group g, position i
                    // of that group — resolves to the grouped fresh name
                    // `_Param_{g}_{i}_BatchGen_` (which the sweeper renumbers
                    // along with the generated names). Unlike `@N` it is
                    // stable across array-dispatch impls (a group absent from
                    // an impl errors here instead of silently shifting).
                    if let Some((g, pos)) = s.split_once('_')
                        && let (Ok(g), Ok(pos)) =
                            (g.parse::<usize>(), pos.parse::<usize>())
                    {
                        let target = format!("_Param_{}_{}_BatchGen_", g, pos);
                        let Some(name) =
                            impl_names.iter().find(|n| n.to_string() == target)
                        else {
                            return Err(compile_err!(
                                "batch-impl: `@{}` in a where predicate — this \
                                 impl has no group {} position {} (grouped \
                                 fresh names are `_Param_{{g}}_{{i}}_BatchGen_`; \
                                 use `@N` for the impl's document-order fresh)",
                                s,
                                g,
                                pos
                            ));
                        };
                        out.extend(name.clone());
                        i += 2;
                        continue;
                    }
                    return Err(compile_error_str(
                        "batch-impl: `@` in a where predicate must be followed by \
                         a position digit (e.g. `@0` or `@0_1`)",
                        tokens[i].span(),
                    ));
                }
                _ => {
                    return Err(compile_error_str(
                        "batch-impl: `@` in a where predicate must be a position digit (e.g. `@0` or `@0_1`)",
                        tokens[i].span(),
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
                        Some(Ty::new(
                            proc_macro2::Span::call_site(),
                            TyPrimitive(quote!(usize)).into(),
                        )),
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
