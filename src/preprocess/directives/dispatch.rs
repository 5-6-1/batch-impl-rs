//! Directive dispatch and the `#fill` / `#delegate` / single-item expansions.
//!
//! This module owns the **dispatch table** of the directive system: it turns a
//! `#name(...)` token sequence into the right expansion, while the individual
//! building blocks live in the sibling files ([`trait_items`], [`name_list`],
//! [`delegate_args`], [`blanket`]). `#blanket` also lives in [`blanket`], but is
//! dispatched from here.
//!
//! ## Directive syntax
//!
//! | Directive | Syntax | Effect |
//! |-----------|--------|--------|
//! | single item | `#name{body}` | `{fn method(sig) { body }}` or `{const NAME: Type = body;}` or `{type Name = body;}` |
//! | fill | `#fill(args){body}` | `{fn m1(sig){body} fn m2(sig){body} ...}` |
//! | delegate | `#delegate(args){target}` | `{fn m1(sig){(target).m1(args)} ...}` |
//! | blanket | `#blanket(args){wrapper list}` | multiple complete specs (see [`blanket::expand_blanket`]) |
//!
//! Expansion output: existing directives produce exactly one `{...}` group
//! (attachable to a type or standalone as a spec); `#blanket` produces
//! multiple specs that can only stand alone.
//!
//! `@all` in `args` means all items of the trait (fn + const + type),
//! `@all_methods` only Fn methods, `@all_constants` only consts, `@all_types`
//! only types.

use proc_macro2::{Group, Ident, TokenStream, TokenTree};
use quote::quote;
use syn::ItemTrait;
use syn::parse::Parser;

use super::*;
use crate::util::compile_err;

/// Dispatches to the expansion functions; output contract in
/// [`expand_tokens`](crate::preprocess::expand_tokens).
pub(crate) fn expand_directive(
    name: &Ident, tokens: &[TokenTree], i: usize, trait_def: &ItemTrait,
    trait_full_path: &TokenStream,
) -> Result<(Vec<TokenTree>, usize), TokenStream> {
    if let Some(TokenTree::Group(args)) = tokens.get(i + 2) {
        match args.delimiter() {
            delimiter![{}] => {
                // `#name{body}` — the item name directly followed by
                // `{body}` (works for fn / const / type). No builtin-typo
                // guard here: `name` is a trait item name, and a method /
                // const / type may legitimately be called `fill`, `delegate`
                // or `blanket` (or a close variant) — the typo guard is only
                // meaningful for open-extension names (the `_` arm below).
                expand_single(name, args, trait_def).map(|tt| (vec![tt], 3))
            }
            _ => {
                // `#cmd(args){body}` — name + parenthesized args + {body}
                let Some(TokenTree::Group(body)) = tokens.get(i + 3) else {
                    return Err(compile_err!(
                        "`#{}` must be followed by `(args)` or `[args]` + \
                         `{{body}}` (or directly `{{body}}`)",
                        name
                    ));
                };
                if body.delimiter() != delimiter![{}] {
                    return Err(compile_err!(
                        "`#{}` must be followed by `(args)` or `[args]` + \
                         `{{body}}` (or directly `{{body}}`)",
                        name
                    ));
                }
                let consumed = 4;
                match name.to_string().as_str() {
                    "fill" => expand_fill(args, body, trait_def).map(|tt| (vec![tt], consumed)),
                    "delegate" => {
                        expand_delegate(args, body, trait_def).map(|tt| (vec![tt], consumed))
                    }
                    "blanket" => expand_blanket(args, body, trait_def, trait_full_path)
                        .map(|v| (v, consumed)),
                    // Open extension: `#name(args){body}` → a **top-level**
                    // macro call `{ ! name!{(args){body} trait_def} }` — the
                    // `!` prefix marks top-level emission: codegen strips it,
                    // prepends the spec body (target + preceding blocks,
                    // merged in chain order) to the macro input, and emits
                    // the call at top level (no impl generated). The macro
                    // receives `{spec}(args){body} trait` and generates
                    // arbitrary items (the same lineage as `#fill`/`#delegate`
                    // — the "read trait → generate" logic is the user's).
                    _ => {
                        let inner = quote! {
                            #name ! { #args #body #trait_def }
                        };
                        Ok((vec![Group::new(delimiter![{}], quote!(! #inner)).into()], consumed))
                    }
                }
            }
        }
    } else {
        Err(compile_err!(
            "`#{}` must be followed by `(args)` / `[args]` or a code \
             block `{{body}}`",
            name
        ))
    }
}

/// `#name{body}` expands to an implementation body matching that item type
/// (see the table above).
///
/// Looks up the item by `name` in the trait definition; `build_from_item`
/// emits the output automatically by item type.
fn expand_single(
    method_name: &Ident, body: &Group, trait_def: &ItemTrait,
) -> Result<TokenTree, TokenStream> {
    let item = get_trait_item(trait_def, method_name)?;
    Ok(Group::new(delimiter![{}], build_from_item(item, &body.stream())).into())
}

/// Common skeleton for multi-item directive expansion: parse the method-name
/// list → build an implementation per item → pack into a `{...}` group.
/// `build` builds the implementation body per item (may error, e.g. a non-fn
/// item / destructuring params in `#delegate`).
fn expand_many(
    args_group: &Group, trait_def: &ItemTrait,
    build: impl Fn(&Ident, &syn::TraitItem) -> Result<TokenStream, TokenStream>,
) -> Result<TokenTree, TokenStream> {
    let method_names =
        parse_names_from_tokens(&args_group.stream().into_iter().collect::<Vec<_>>(), trait_def)?;
    let mut methods = TokenStream::new();
    for name in &method_names {
        let item = get_trait_item(trait_def, name)?;
        methods.extend(build(name, item)?);
    }
    Ok(Group::new(delimiter![{}], methods).into())
}

/// `#fill(args){body}` → `{fn m1(sig){body} fn m2(sig){body} ...}`
///
/// `args` is a comma-separated item-name list, or `@all` (meaning all items).
/// Supports three item kinds: fn, const, type.
/// For each item, the signature/type is read from the trait definition and
/// `body` is used as the implementation.
fn expand_fill(
    args_group: &Group, body: &Group, trait_def: &ItemTrait,
) -> Result<TokenTree, TokenStream> {
    let body_stream = body.stream();
    expand_many(args_group, trait_def, |_name, item| Ok(build_from_item(item, &body_stream)))
}

/// `#delegate(args){target}` → `{fn m1(sig){(target).m1(params)} ...}`
///
/// Generates a delegation call per method: skips the `self` argument and
/// forwards the remaining arguments as-is. Non-identifier parameter patterns
/// (`_`, tuple patterns like `(a, b)` — legal when the trait method has a
/// default body — or any other pattern) are renamed to `arg0`, `arg1`, ...
/// in both the copied signature and the delegation call, so they can be
/// forwarded by name.
///
/// **Renaming**: an element `size=len` delegates the trait's `size` method
/// to the target's `len` method (the call body uses `len`; the signature
/// keeps `size`) — the `#[call(...)]` mechanism of the `delegate` crate, in
/// the DSL's `=` binding spelling. Mixes freely with plain names and `@all`
/// (`#delegate(@all, size=len){...}`).
fn expand_delegate(
    args_group: &Group, target: &Group, trait_def: &ItemTrait,
) -> Result<TokenTree, TokenStream> {
    let target_stream = target.stream();
    let arg_tokens = args_group.stream().into_iter().collect::<Vec<_>>();
    // Split off `ident=ident` rename mappings; the remaining tokens (plain
    // names / `@all`) go through the standard name-list parser.
    let mut renames: std::collections::HashMap<String, String> = Default::default();
    let mut method_tokens: Vec<TokenTree> = vec![];
    for chunk in crate::parse::split_at_depth0(&arg_tokens, ',') {
        if let Some(eq) =
            chunk.iter().position(|t| matches!(t, TokenTree::Punct(p) if p.as_char() == '='))
        {
            // `eq == 0` (`#delegate(=foo)`) is a missing left side — the
            // checked_sub keeps the no-panic promise (a raw `eq - 1` would
            // overflow in debug builds).
            let from_ident = match eq.checked_sub(1).and_then(|i| chunk.get(i)) {
                Some(TokenTree::Ident(id)) => id.clone(),
                _ => {
                    return Err(compile_err!(
                        "batch-impl: #delegate rename `X = Y` needs identifiers on \
                         both sides (e.g. `#delegate(size = len)`)"
                    ));
                }
            };
            let to_ident = match chunk.get(eq + 1) {
                Some(TokenTree::Ident(id)) if eq + 2 == chunk.len() => id.clone(),
                _ => {
                    return Err(compile_err!(
                        "batch-impl: #delegate rename `X = Y` needs a single \
                         identifier on the right (e.g. `#delegate(size = len)`)"
                    ));
                }
            };
            // A trait method can be bound to only one target method: a
            // second `foo = ...` for the same `foo` is a conflict, not a
            // merge (the merged set may also hold `foo` via `@all` — that
            // overlap is deduplicated downstream by the name-list parser,
            // keeping the first occurrence).
            if renames.contains_key(&from_ident.to_string()) {
                return Err(compile_err!(
                    "batch-impl: #delegate method `{}` is renamed twice \
                     (`{}=...` appears more than once); a method can delegate \
                     to only one target",
                    from_ident,
                    from_ident
                ));
            }
            renames.insert(from_ident.to_string(), to_ident.to_string());
            method_tokens.push(from_ident.into());
        } else {
            method_tokens.extend(chunk.iter().cloned());
        }
    }
    let method_names = parse_names_from_tokens(&method_tokens, trait_def)?;
    let mut methods = TokenStream::new();
    for name in &method_names {
        let item = get_trait_item(trait_def, name)?;
        let syn::TraitItem::Fn(f) = item else {
            return Err(compile_err!(
                "batch-impl: #delegate only works on methods; `{}` in trait \
                 `{}` is not a method",
                trait_def.ident,
                name
            ));
        };
        // The delegated target method: the rename mapping or the same name.
        let call_name = match renames.get(&name.to_string()) {
            Some(c) => Ident::new(c, name.span()),
            None => name.clone(),
        };
        let mut sig = f.sig.clone();
        // Only patterns that cannot be used directly as an expression need
        // renaming: `_` has no binding, `ref x` / guards / `x @ pat` are
        // pattern-only tokens (checked recursively, so `(ref x, y)` is
        // caught too). Everything else keeps its pattern — its token stream
        // works as an expression in the call (`(a, b)` binds `a`/`b`, and
        // `(a, b)` rebuilds the tuple). Parsed via syn (`arg{i}` is always a
        // valid identifier pattern).
        let mut arg_idx = 0usize;
        for input in &mut sig.inputs {
            if let syn::FnArg::Typed(pat_type) = input
                && !pat_is_forwardable(&pat_type.pat)
            {
                // `arg{i}` is always a valid identifier pattern; the
                // fallback turns a hypothetical failure into a compile
                // error instead of a panic (no-panic promise).
                let arg_name = format!("arg{}", arg_idx);
                pat_type.pat = syn::Pat::parse_single
                    .parse_str(&arg_name)
                    .map_err(|_| {
                        compile_err!(
                            "batch-impl: internal error: generated argument name \
                             `{}` is not a valid identifier pattern",
                            arg_name
                        )
                    })?
                    .into();
                arg_idx += 1;
            }
        }
        let call_args = collect_call_args(&sig).map_err(|pat| {
            compile_err!(
                "batch-impl: #delegate method `{}::{}` param `{}` cannot be \
                 forwarded (unsupported parameter pattern); please rename it \
                 to a plain identifier",
                trait_def.ident,
                name,
                pat
            )
        })?;
        let body = quote! { (#target_stream) . #call_name ( #(#call_args),* ) };
        methods.extend(build_from_item_sig(item, Some(&sig), &body));
    }
    Ok(Group::new(delimiter![{}], methods).into())
}
