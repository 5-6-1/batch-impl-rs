//! Preprocessing layer: token rewriters (one pass per file).
//!
//! - [`angle`]: pairs `<>` into angle groups (entry transformation);
//! - [`consts`]: expands `@` constants (macro-meta layer, lexical substitution);
//! - [`mod`](self): expands `#` directives (fill/delegate/blanket/open extension);
//! - [`where_process`]: rewrites bare `where` predicates;
//! - [`empty_generics`]: copies `A<>`;
//! - [`helpers`]: directive-argument parsing helpers.
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
pub(crate) mod consts_ctx;
pub(crate) mod empty_generics;
pub(crate) mod helpers;
pub(crate) mod where_process;

pub(crate) use angle::*;
pub(crate) use consts::*;
pub(crate) use consts_ctx::*;
pub(crate) use empty_generics::*;
pub(crate) use helpers::*;
pub(crate) use where_process::*;

mod blanket;
pub(crate) use blanket::expand_blanket;

use proc_macro2::{Group, Ident, TokenStream, TokenTree};
use quote::quote;
use syn::ItemTrait;

use crate::util::Cursor;
use crate::util::{compile_err, compile_error_str};

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
    cursor: &mut Cursor, trait_def: &ItemTrait, trait_full_path: &TokenStream,
) -> Result<Vec<TokenTree>, TokenStream> {
    let mut result = vec![];
    while !cursor.at_end() {
        if cursor.is_punct('#')
            && let Some(TokenTree::Ident(name)) = cursor.peek_at(1)
        {
            result.extend(expand_directive(
                name,
                cursor,
                trait_def,
                trait_full_path,
            )?);
            continue;
        }
        // The loop condition guarantees non-at_end; break is only defensive
        let Some(tt) = cursor.peek() else {
            break;
        };
        // Only `[...]` is expanded recursively (`ident![...]` / `#[...]`
        // passthrough, aligned with the angle_collect guard)
        if let TokenTree::Group(g) = tt
            && g.delimiter() == delimiter![[]]
            && !cursor.prev_bracket_passthrough()
        {
            let inner = expand_tokens(
                &mut Cursor::new(&g.stream().into_iter().collect::<Vec<_>>()),
                trait_def,
                trait_full_path,
            )?;
            let new_group = Group::new(g.delimiter(), inner.into_iter().collect());
            result.push(new_group.into());
            cursor.bump();
        } else {
            result.push(tt.clone());
            cursor.bump();
        }
    }
    Ok(result)
}

/// Dispatches to the expansion functions; output contract in [`expand_tokens`].
fn expand_directive(
    name: &Ident, cursor: &mut Cursor, trait_def: &ItemTrait,
    trait_full_path: &TokenStream,
) -> Result<Vec<TokenTree>, TokenStream> {
    if let Some(TokenTree::Group(args)) = cursor.peek_at(2) {
        match args.delimiter() {
            delimiter![{}] => {
                // `#name{body}` — the item name directly followed by
                // `{body}` (works for fn / const / type)
                cursor.bump(); // #
                cursor.bump(); // method_name
                cursor.bump(); // {body}
                expand_single(name, args, trait_def).map(|tt| vec![tt])
            }
            _ => {
                // `#cmd(args){body}` — name + parenthesized args + {body}
                let body_tt = cursor.peek_at(3);
                let Some(TokenTree::Group(body)) = body_tt else {
                    return Err(compile_err!(
                        "`#{}` must be followed by `(args)` + `{{body}}` or \
                         directly `{{body}}`",
                        name
                    ));
                };
                if body.delimiter() != delimiter![{}] {
                    return Err(compile_err!(
                        "`#{}` must be followed by `(args)` + `{{body}}` or \
                         directly `{{body}}`",
                        name
                    ));
                }
                cursor.bump(); // #
                cursor.bump(); // name
                cursor.bump(); // (args)
                cursor.bump(); // {body}
                match name.to_string().as_str() {
                    "fill" => expand_fill(args, body, trait_def).map(|tt| vec![tt]),
                    "delegate" => {
                        expand_delegate(args, body, trait_def).map(|tt| vec![tt])
                    }
                    "blanket" => {
                        expand_blanket(args, body, trait_def, trait_full_path)
                    }
                    // Open extension: `#name(args){body}` →
                    // `{ name!{(args){body} trait_def} }`, a function-like
                    // macro call in the impl body (attached usage) or at top
                    // level (standalone usage). Same lineage as
                    // `#fill`/`#delegate`: the "read trait → generate fn
                    // definitions" implementation is handed to the user's
                    // macro of the same name — it parses args / body / trait
                    // and produces impl items.
                    _ => {
                        let inner = quote! {
                            #name ! { #args #body #trait_def }
                        };
                        Ok(vec![Group::new(delimiter![{}], inner).into()])
                    }
                }
            }
        }
    } else {
        Err(compile_err!(
            "`#{}` must be followed by parenthesized args `(args)` or a code \
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
/// forwards the remaining arguments as-is.
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
        let sig = f.sig.clone();
        let call_args = collect_call_args(&sig).map_err(|pat| {
            compile_err!(
                "batch-impl: #delegate method `{}::{}` param `{}` cannot be \
                 forwarded: only `self` and plain identifier patterns are \
                 supported",
                trait_def.ident,
                name,
                pat
            )
        })?;
        let body = quote! { (#target_stream) . #name ( #(#call_args),* ) };
        Ok(build_from_item(item, &body))
    })
}
