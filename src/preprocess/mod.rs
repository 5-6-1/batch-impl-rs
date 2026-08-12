//! Preprocessing layer: token rewriters (one pass per file).
//!
//! - [`angle`]: pairs `<>` into angle groups (entry transformation);
//! - [`consts`]: expands `@` constants (macro-meta layer, lexical substitution);
//! - [`mod`](self): expands `#` directives (fill/delegate/blanket/open extension);
//! - [`where_process`]: rewrites bare `where` predicates;
//! - [`empty_generics`]: copies `A<>`;
//!
//! The passes are called by the entry layer in a fixed order; `mod.rs`
//! aggregates the re-exports, referenced as `crate::preprocess::X`.

// ============================================================
// Delimiter spelling macro
// ============================================================

/// Delimiter spelling macro: unifies `Delimiter::*` literals as the source
/// delimiter spelling (calls always use `[]`) — `delimiter![{}]` /
/// `delimiter![[]]` / `delimiter![()]` correspond one-to-one with the source.
///
/// proc-macro2's `Delimiter` has no "angle" variant, so `<>` must borrow
/// `Delimiter::None` — but `None` is also the spelling of a real
/// "transparent group". To avoid the ambiguity, the macro distinguishes two
/// spellings:
/// - `delimiter![<>]`: the **angle-group** carrier (`angle_collect` pairing output);
/// - `delimiter![none]`: a **real transparent group** (macro-variable
///   `$var:ty` expansion output, whose content is DSL tokens to flatten).
///
/// Both expand to the same value (`Delimiter::None`), so they cannot be two
/// arms of the same `match` (would report unreachable pattern); actual usage
/// is spread across mutually exclusive contexts, with no conflict.
macro_rules! delimiter {
    ({}) => {
        ::proc_macro2::Delimiter::Brace
    };
    ([]) => {
        ::proc_macro2::Delimiter::Bracket
    };
    (()) => {
        ::proc_macro2::Delimiter::Parenthesis
    };
    (<>) => {
        ::proc_macro2::Delimiter::None
    };
    (none) => {
        ::proc_macro2::Delimiter::None
    };
}

pub(crate) mod angle;
pub(crate) mod consts;
pub(crate) mod directives;
pub(crate) mod empty_generics;
pub(crate) mod where_process;

pub(crate) use angle::*;
pub(crate) use consts::*;
pub(crate) use directives::*;
pub(crate) use empty_generics::*;
pub(crate) use where_process::*;

use proc_macro2::{Group, Ident, TokenStream, TokenTree};
use quote::quote;
use syn::ItemTrait;
use syn::parse::Parser;

use crate::util::{bracket_is_passthrough, compile_err, is_punct};

// ============================================================
// Directive preprocessing
// ============================================================

/// Directive preprocessing entry: scans the token stream and expands `#`
/// directives.
///
/// Supported only by `#[batch_impl]` / `#[batch_impl_only]` (needs the trait
/// definition to read method signatures). `batch_trait!` does not call this
/// function (no trait definition available).
///
/// ## Directive syntax
///
/// | Directive | Syntax | Effect |
/// |-----------|--------|--------|
/// | single item | `#name{body}` | `{fn method(sig) { body }}` or `{const NAME: Type = body;}` or `{type Name = body;}` |
/// | fill | `#fill(args){body}` | `{fn m1(sig){body} fn m2(sig){body} ...}` |
/// | delegate | `#delegate(args){target}` | `{fn m1(sig){(target).m1(args)} ...}` |
/// | blanket | `#blanket(args){wrapper list}` | multiple complete specs (see [`expand_blanket`]) |
///
/// Expansion output: existing directives produce exactly one `{...}` group
/// (attachable to a type or standalone as a spec); `#blanket` produces
/// multiple specs that can only stand alone (self-contained
/// generics/target/delegation; see the attachment semantics under
/// "syntax-domain isolation" in architecture.md).
///
/// `@all` in `args` means all items of the trait (fn + const + type),
/// `@all_methods` only Fn methods, `@all_constants` only consts, `@all_types`
/// only types.
///
/// ## Recursion rules
///
/// Only the contents of `[...]` (Bracket) groups are expanded recursively;
/// `(...)` and `{...}` are not, to avoid wandering into directive args or
/// bodies.
pub(crate) fn expand_tokens(
    tokens: &[TokenTree], trait_def: &ItemTrait, trait_full_path: &TokenStream,
) -> Result<Vec<TokenTree>, TokenStream> {
    let mut result = vec![];
    let mut i = 0;
    while i < tokens.len() {
        if is_punct(&tokens[i], '#')
            && let Some(TokenTree::Ident(name)) = tokens.get(i + 1)
        {
            let (out, consumed) =
                expand_directive(name, tokens, i, trait_def, trait_full_path)?;
            result.extend(out);
            i += consumed;
            continue;
        }
        // Only `[...]` is expanded recursively (`ident![...]` / `#[...]`
        // passthrough, aligned with the angle_collect guard)
        if let TokenTree::Group(g) = &tokens[i]
            && g.delimiter() == delimiter![[]]
            && !bracket_is_passthrough(tokens, i)
        {
            let inner = expand_tokens(
                &g.stream().into_iter().collect::<Vec<_>>(),
                trait_def,
                trait_full_path,
            )?;
            let new_group = Group::new(g.delimiter(), inner.into_iter().collect());
            result.push(new_group.into());
        } else {
            result.push(tokens[i].clone());
        }
        i += 1;
    }
    Ok(result)
}

/// Dispatches to the expansion functions; output contract in [`expand_tokens`].
fn expand_directive(
    name: &Ident, tokens: &[TokenTree], i: usize, trait_def: &ItemTrait,
    trait_full_path: &TokenStream,
) -> Result<(Vec<TokenTree>, usize), TokenStream> {
    if let Some(TokenTree::Group(args)) = tokens.get(i + 2) {
        match args.delimiter() {
            delimiter![{}] => {
                // `#name{body}` — the item name directly followed by
                // `{body}` (works for fn / const / type)
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
                    "fill" => expand_fill(args, body, trait_def)
                        .map(|tt| (vec![tt], consumed)),
                    "delegate" => expand_delegate(args, body, trait_def)
                        .map(|tt| (vec![tt], consumed)),
                    "blanket" => {
                        expand_blanket(args, body, trait_def, trait_full_path)
                            .map(|v| (v, consumed))
                    }
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
                        Ok((
                            vec![Group::new(delimiter![{}], quote!(! #inner)).into()],
                            consumed,
                        ))
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
    let method_names = parse_names_from_tokens(
        &args_group.stream().into_iter().collect::<Vec<_>>(),
        trait_def,
    )?;
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
    expand_many(args_group, trait_def, |_name, item| {
        Ok(build_from_item(item, &body_stream))
    })
}

/// `#delegate(args){target}` → `{fn m1(sig){(target).m1(params)} ...}`
///
/// Generates a delegation call per method: skips the `self` argument and
/// forwards the remaining arguments as-is. Non-identifier parameter patterns
/// (`_`, tuple patterns like `(a, b)` — legal when the trait method has a
/// default body — or any other pattern) are renamed to `arg0`, `arg1`, ...
/// in both the copied signature and the delegation call, so they can be
/// forwarded by name.
fn expand_delegate(
    args_group: &Group, target: &Group, trait_def: &ItemTrait,
) -> Result<TokenTree, TokenStream> {
    let target_stream = target.stream();
    expand_many(args_group, trait_def, |name, item| {
        let syn::TraitItem::Fn(f) = item else {
            return Err(compile_err!(
                "batch-impl: #delegate only works on methods; `{}` in trait \
                 `{}` is not a method",
                trait_def.ident,
                name
            ));
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
                pat_type.pat = syn::Pat::parse_single
                    .parse_str(&format!("arg{}", arg_idx))
                    .expect(
                        "generated arg names are always valid identifier patterns",
                    )
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
        let body = quote! { (#target_stream) . #name ( #(#call_args),* ) };
        Ok(build_from_item_sig(item, Some(&sig), &body))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::TokenStream;

    /// Inputs whose Bracket/Paren/Brace groups must be treated as
    /// passthrough by every recursive entry point (`ident!{...}` /
    /// `ident![...]` / `ident!(...)` macro bodies and `#[...]` attributes
    /// contain arbitrary Rust — comparisons, `#name` directives, `@`
    /// constants, `;` — none of which is DSL).
    fn passthrough_inputs() -> Vec<&'static str> {
        vec![
            "m![a < b]",
            "m!(a < b)",
            "m![#foo{1}]",
            "#[a < b]",
            "#[#zzz{1}]",
            "m![@u*]",
            "m![where a b]",
            "m![a; b]",
        ]
    }

    /// All four recursive entries (angle_collect / expand_consts /
    /// expand_tokens / where_process) must agree on passthrough: none of
    /// them enters a macro body or attribute (regression guard for 0.5.7,
    /// where a missing `#[...]` guard let `#name` directives inside an
    /// attribute be wrongly expanded).
    #[test]
    fn passthrough_guard_consistency() {
        let trait_def: syn::ItemTrait = syn::parse_quote!(
            trait T {
                fn m(&self) -> u32;
            }
        );
        let trait_full_path = quote!(T);
        let ctx = ConstCtx::Trait { user_table: &UserConsts::new() };
        for s in passthrough_inputs() {
            let v = s.parse::<TokenStream>().unwrap().into_iter().collect::<Vec<_>>();
            assert!(angle_collect(&v).is_ok(), "angle_collect: {s}");
            assert!(expand_consts(&v, ctx).is_ok(), "expand_consts: {s}");
            assert!(
                expand_tokens(&v, &trait_def, &trait_full_path).is_ok(),
                "expand_tokens: {s}"
            );
            assert!(where_process(&v).is_ok(), "where_process: {s}");
        }
        // Control: WITHOUT the `!`/`#` marker the same content IS entered and
        // errors (proves the test distinguishes passthrough from recursion).
        let bare = "(a < b)".parse::<TokenStream>().unwrap();
        assert!(
            angle_collect(&bare.into_iter().collect::<Vec<_>>()).is_err(),
            "plain paren groups are entered, not passed through"
        );
    }
}
