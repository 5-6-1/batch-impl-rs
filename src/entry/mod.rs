//! Shared implementation of the macro entry points: attribute macro expansion,
//! `batch_trait!` segment expansion, and the common pipeline.
//!
//! Error handling split: this (entry) layer propagates `Result<_, TokenStream>` via
//! `?` and builds errors uniformly with `compile_error_str`; the DSL layer (parse/
//! apply/codegen) passes `Ty::Error` through the AST chain — the two mechanisms serve
//! different layers and are never merged.
//!
//! Common pipeline [`run_pipeline`] = DSL parse/expand → generate impl → restore angle
//! brackets. `angle_collect` and the bare `where` rewrite are **not in** the pipeline:
//! pairing is destructive (re-collecting a paired group flattens it as a real None
//! group), and the where rewrite must precede `A<>` expansion (`Foo<>` in predicates
//! must pass through) — both are invoked once by the two entry points in order.

use proc_macro2::{Ident, TokenStream, TokenTree};
use quote::quote;
use syn::ItemTrait;

use crate::analyze::TraitBounds;
use crate::ast::{Op, reset_fresh_counter};
use crate::preprocess::{
    angle_collect, expand_empty_trait_generics, expand_tokens, render_angles,
    where_process,
};
use crate::util::{Cursor, compile_error_str};
use path_prefix::try_parse_path_prefix;

use crate::entry::driver::parse_batch_trait_entry;

pub(crate) mod driver;
pub(crate) mod path_prefix;
mod preprocess_test;
pub(crate) use preprocess_test::preprocess_test;

/// Common pipeline: DSL parse/expand → generate impl → restore angle brackets.
///
/// `tokens` must already be paired by `angle_collect` and bare-`where` rewritten
/// (see module docs); `top_level` controls the stop semantics of the spec list.
/// Errors are returned via `Err` as a `compile_error!` stream.
fn run_pipeline(
    tokens: &[TokenTree], top_level: Op, trait_full_path: &TokenStream,
    trait_last_ident: &Ident, is_unsafe: bool, start_trait: Option<ItemTrait>,
    trait_bounds: &TraitBounds,
) -> Result<TokenStream, TokenStream> {
    let mut cursor = Cursor::new(tokens);
    let impls = parse_batch_trait_entry(
        &mut cursor,
        top_level,
        trait_full_path,
        trait_last_ident,
        is_unsafe,
        start_trait,
        trait_bounds,
    );
    // Exit conversion: restore angle-bracket groups to flat `<...>` tokens (see render_angles)
    Ok(render_angles(impls))
}

/// Shared implementation of the two attribute macros (errors via `compile_error!` streams)
/// Parameters use proc_macro2 types: unit tests (fuzz) can call directly without a proc-macro
/// runtime; the attribute macro entry points (lib.rs) convert at expansion time.
pub(crate) fn expand_attr_macro(
    attr: TokenStream, trait_item: ItemTrait, include_trait: bool,
) -> Result<TokenStream, TokenStream> {
    reset_fresh_counter();
    let trait_name = trait_item.ident.clone();
    let attr_vec = attr.into_iter().collect::<Vec<_>>();

    // `#[batch_impl_only]`-specific: if attr starts with a `# Path: ` shape
    // (`#` + `Ident (:: Ident)*` + `:`), that path is used as the external trait path
    // and the rest of attr is the DSL spec. `#[batch_impl]` does not support this
    // prefix (it emits the local trait definition, so a path prefix is meaningless).
    // This runs before `@` expansion: `@trait` needs trait_full_path (batch_impl_only
    // expands to the external path, batch_impl to the local name).
    let prefix = (!include_trait).then(|| try_parse_path_prefix(&attr_vec)).flatten();
    let (trait_full_path, trait_last_ident, rest_tokens) = match prefix {
        Some((path, last_ident, rest)) => {
            // The path prefix's last ident must match the local dummy trait name,
            // otherwise `Trait<T>` matching in the subsequent DSL would fail.
            match last_ident {
                Some(id) if id == trait_name => {
                    let path_ts = path.into_iter().collect();
                    // Borrow the local trait_name here as the matching ident
                    // (already verified to share the name with the path's last segment).
                    (path_ts, trait_name.clone(), rest)
                }
                Some(id) => {
                    let msg = format!(
                        "batch-impl: path prefix `#...{}` \
                                 has a trailing ident that differs from the trait \
                                 name `{}`; the two must be identical",
                        id, trait_name,
                    );
                    return Err(compile_error_str(&msg, id.span()));
                }
                None => {
                    let msg = "batch-impl: expected at least one ident after the \
                             path prefix `#` as the trait path";
                    return Err(compile_error_str(
                        msg,
                        proc_macro2::Span::call_site(),
                    ));
                }
            }
        }
        None => (quote![#trait_name], trait_name.clone(), attr_vec.clone()),
    };

    // Outermost macro-meta layer: `@` constant expansion (pure lexical substitution)
    // precedes `<>` pairing — output may contain flat `<...>` (e.g. `@map = HashMap<u32, String>`
    // values) that angle_collect must pair uniformly; reversed, `Vec<@inner>`'s
    // `@inner` is paired into the `<>` group and expand_consts never enters it, leaving
    // residue behind (observed compile error).
    let rest_tokens = crate::preprocess::expand_consts(
        &rest_tokens,
        crate::preprocess::ConstCtx::Attribute {
            trait_def: &trait_item,
            trait_full_path: &trait_full_path,
        },
    )?;
    // Entry conversion: flatten None groups + pair `<...>` (see angle_collect)
    let rest_tokens = angle_collect(&rest_tokens)?;

    let expanded = expand_tokens(&rest_tokens, &trait_item, &trait_full_path)?;
    // New bare `where predicate {body}` syntax → uniformly rewritten to legacy `where{predicate}`
    // (before `A<>` expansion: `Foo<>` inside predicates must pass through, not be expanded)
    let expanded = where_process(&expanded)?;
    let is_unsafe = trait_item.unsafety.is_some();
    let trait_bounds = crate::analyze::extract_trait_bounds(&trait_item);
    // `A<>`: copy the trait generics (args and bounds all come from the trait definition,
    // including where predicates), so the expansion is fully equivalent to handwritten code.
    let expanded =
        expand_empty_trait_generics(&expanded, &trait_item, &trait_bounds)?;
    let start_trait = if include_trait { trait_item.into() } else { None };
    run_pipeline(
        &expanded,
        Op::Comma,
        &trait_full_path,
        &trait_last_ident,
        is_unsafe,
        start_trait,
        &trait_bounds,
    )
}

/// Segment-level `@trait` → this segment's full trait path (batch_trait!-specific; constant
/// values like `<T>@trait<T>` keep `@trait` via lazy expansion, replaced here per segment —
/// each segment uses its own name).
fn replace_segment_trait(
    tokens: Vec<TokenTree>, trait_full_path: &TokenStream,
) -> Result<Vec<TokenTree>, TokenStream> {
    let mut out = vec![];
    let mut i = 0;
    while i < tokens.len() {
        if let TokenTree::Punct(p) = &tokens[i]
            && p.as_char() == '@'
            && let Some(TokenTree::Ident(id)) = tokens.get(i + 1)
            && id == "trait"
        {
            out.extend(trait_full_path.clone());
            i += 2;
        } else if let TokenTree::Group(g) = &tokens[i] {
            // Recurse into groups (where{...} predicates and type groups):
            // segment-level `@trait` must reach every DSL structure, not
            // just the top level.
            let inner = g.stream().into_iter().collect::<Vec<_>>();
            let inner = replace_segment_trait(inner, trait_full_path)?;
            out.push(
                proc_macro2::Group::new(g.delimiter(), inner.into_iter().collect())
                    .into(),
            );
            i += 1;
        } else {
            out.push(tokens[i].clone());
            i += 1;
        }
    }
    Ok(out)
}

/// Actual expansion of `batch_trait!` (errors returned as `compile_error!` token streams)
pub(crate) fn expand_batch_trait(
    input: proc_macro::TokenStream,
) -> Result<proc_macro::TokenStream, TokenStream> {
    reset_fresh_counter();
    let tokens = TokenStream::from(input).into_iter().collect::<Vec<_>>();
    // Global preprocessing: `@` constants (outermost macro-meta layer) → angle-bracket
    // pairing → bare where rewrite (done once before segmenting; `@` precedes pairing:
    // the expansion may contain flat `<...>` that angle_collect must pair uniformly —
    // reversed, `Vec<@inner>`'s `@inner` enters the group and is never expanded; observed).
    let (tokens, user_consts) = crate::preprocess::collect_user_consts(&tokens)?;
    let tokens = crate::preprocess::expand_consts(
        &tokens,
        crate::preprocess::ConstCtx::Trait { user_table: &user_consts },
    )?;
    let tokens = angle_collect(&tokens)?;
    let tokens = where_process(&tokens)?;
    let mut cursor = Cursor::new(&tokens);
    let mut result = quote![];
    loop {
        // Fresh-generator group ids are DSL-local per segment (each segment
        // generates independent impl sets).
        reset_fresh_counter();
        // Skip leading `;` (allows consecutive semicolons and a trailing one)
        while cursor.is_punct(';') {
            cursor.bump();
        }
        if cursor.at_end() {
            break;
        }

        // `unsafe` prefix: mark all impls in this segment as unsafe impls
        let is_unsafe = if matches!(cursor.peek(), Some(TokenTree::Ident(id)) if *id == "unsafe")
        {
            cursor.bump();
            true
        } else {
            false
        };

        // Collect the trait path (stop at `:`; collect `::` path separators too).
        // Angle brackets were paired into opaque groups by angle_collect, so no `<>` depth tracking.
        let path_start = cursor.pos();
        while let Some(token) = cursor.peek() {
            match token {
                TokenTree::Punct(p) if p.as_char() == ':' => {
                    if cursor.is_single_colon() {
                        break;
                    } else {
                        cursor.bump();
                        cursor.bump();
                    }
                }
                _ => cursor.bump(),
            }
        }
        let trait_path = cursor.slice_since(path_start);
        if trait_path.is_empty() {
            return Err(compile_error_str(
                "batch_trait! expects a trait name",
                cursor.span(),
            ));
        }
        // Full trait path: just collect the token stream of trait_path as-is
        let trait_full_path = trait_path.iter().cloned().collect();
        // Take the last ident in the path as the `trait_name` used for matching
        let trait_last_ident =
            match trait_path
                .iter()
                .filter_map(|tt| {
                    if let TokenTree::Ident(id) = tt { id.into() } else { None }
                })
                .next_back()
            {
                Some(ident) => ident,
                None => {
                    return Err(compile_error_str(
                        "batch_trait! expects an ident as the trait name",
                        trait_path
                            .first()
                            .map_or_else(proc_macro2::Span::call_site, |t| t.span()),
                    ));
                }
            };
        if !cursor.is_punct(':') {
            return Err(compile_error_str(
                "batch_trait! expects ':' to separate the trait name and impl-specs",
                cursor.span(),
            ));
        }
        cursor.bump();
        // Segment boundary = first depth-0 `;` (not consumed; skipped by the loop head)
        let spec = cursor.take_segment(&[';']).to_vec();
        // Segment-level `@trait` replacement: batch_trait!'s `@trait` is kept as-is during
        // the constant stage (each segment has a different trait name), expanded here to
        // this segment's full trait path — the `@type_t=<T>@trait<T>` cross-segment reuse
        // scenario (`A: @type_t ...` / `B: @type_t ...`).
        let spec = replace_segment_trait(spec, &trait_full_path)?;
        result.extend(run_pipeline(
            &spec,
            Op::Comma,
            &trait_full_path,
            trait_last_ident,
            is_unsafe,
            None,
            // batch_trait! has no trait definition, so generic bounds cannot be inherited
            &Default::default(),
        )?);
    }
    Ok(result.into())
}
