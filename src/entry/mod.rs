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
use crate::preprocess::consts::ConstCtx;
use crate::preprocess::render_angles;
use crate::util::{Cursor, compile_error_str};
use path_prefix::try_parse_path_prefix;

use crate::entry::driver::parse_batch_trait_entry;

pub(crate) mod driver;
pub(crate) mod impl_entry;
mod impl_fresh;
pub(crate) mod impl_spec;
#[cfg(test)]
mod pack_entry_tests;
pub(crate) mod path_prefix;
mod preprocess_test;
mod preview;
pub(crate) use impl_entry::expand_impl_entry;
pub(crate) use preprocess_test::preprocess_test;
pub(crate) use preview::preview;

/// Common pipeline: DSL parse/expand → generate impl → restore angle brackets.
///
/// `tokens` must already be paired by `angle_collect` and bare-`where` rewritten
/// (see module docs); `top_level` controls the stop semantics of the spec list.
/// Errors are returned via `Err` as a `compile_error!` stream.
// clippy's 7-argument threshold is not useful here: each parameter is one
// distinct input of the entry (tokens, stop semantics, trait path + name,
// unsafety, the trait definition, its bounds, its parameter names), and a
// context struct would only move the list (the same reason as
// `driver::parse_batch_trait_entry` and `impl_spec::assemble_impl`).
#[allow(clippy::too_many_arguments)]
fn run_pipeline(
    tokens: &[TokenTree], top_level: Op, trait_full_path: &TokenStream, trait_last_ident: &Ident,
    is_unsafe: bool, start_trait: Option<ItemTrait>, trait_bounds: &TraitBounds,
    trait_param_names: &[Ident],
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
        trait_param_names,
    );
    // Exit conversion: restore angle-bracket groups to flat `<...>` tokens (see render_angles)
    Ok(render_angles(impls))
}

/// Whether a spec list carries nothing but separators, so the entry re-emits the item unchanged:
/// `#[batch_impl]`, `#[batch_impl()]` and `#[batch_impl(;)]` are the same instruction, and
/// `docs/reference.md` documents all three as the identity. One predicate for both entries and for
/// `batch_preview!` - the preview used to bypass it, reporting a bare `#[batch_impl]` as a missing
/// attribute and `#[batch_impl(;)]` as an empty spec, which is probe D's F1.
pub(crate) fn spec_list_is_empty(attr: &[TokenTree]) -> bool {
    crate::parse::split_at_depth0(attr, ';').iter().all(|spec| spec.is_empty())
}

/// Shared implementation of the two attribute macros (errors via `compile_error!` streams)
/// Parameters use proc_macro2 types: unit tests (fuzz) can call directly without a proc-macro
/// runtime; the attribute macro entry points (lib.rs) convert at expansion time.
pub(crate) fn expand_attr_macro(
    attr: TokenStream, trait_item: ItemTrait, include_trait: bool,
) -> Result<TokenStream, TokenStream> {
    // Nothing to derive: hand the trait back unchanged, exactly as the impl entry does for its
    // own empty list (`entry/impl_entry.rs`). Separators are not content, so `#[batch_impl(;)]`
    // and `#[batch_impl()]` are the same instruction — `docs/reference.md` documents both as
    // "re-emit the item unchanged". This bypasses the pipeline on purpose: the zero-impl gate
    // would otherwise report an empty expansion and turn the documented identity into an error.
    let attr_vec: Vec<TokenTree> = attr.clone().into_iter().collect();
    if spec_list_is_empty(&attr_vec) {
        return Ok(if include_trait { quote!(#trait_item) } else { quote!() });
    }
    let p = prepare_attr_expansion(attr, trait_item, include_trait)?;
    run_pipeline(
        &p.expanded,
        Op::Comma,
        &p.trait_full_path,
        &p.trait_last_ident,
        p.is_unsafe,
        p.start_trait,
        &p.trait_bounds,
        &p.trait_param_names,
    )
}

/// The prepared state of an attribute-macro expansion: everything the shared
/// pipeline needs (paired/expanded tokens + trait context), staged before the
/// parse/expand stage. Extracted from [`expand_attr_macro`] so the preview
/// entry (`batch_preview!`) runs the same preprocessing once and then
/// collects leaves itself instead of rendering immediately.
pub(crate) struct PreparedAttr {
    pub(crate) trait_full_path: TokenStream,
    pub(crate) trait_last_ident: Ident,
    pub(crate) is_unsafe: bool,
    pub(crate) start_trait: Option<ItemTrait>,
    pub(crate) trait_bounds: TraitBounds,
    pub(crate) trait_param_names: Vec<Ident>,
    pub(crate) expanded: Vec<TokenTree>,
}

pub(crate) fn prepare_attr_expansion(
    attr: TokenStream, trait_item: ItemTrait, include_trait: bool,
) -> Result<PreparedAttr, TokenStream> {
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
                    return Err(compile_error_str(msg, proc_macro2::Span::call_site()));
                }
            }
        }
        None => (quote![#trait_name], trait_name.clone(), attr_vec.clone()),
    };

    // Outermost macro-meta layer: `@` constant expansion (pure lexical substitution)
    // precedes `<>` pairing — output may contain flat `<...>` (e.g. `@map = HashMap<u32, String>`
    // values) that angle_collect must pair uniformly; reversed, `Vec<@inner>`'s
    // `@inner` is paired into the `<>` group and expand_consts never enters it, leaving
    // residue behind (observed compile error). Custom `@name=value;` sections are
    // `batch_trait!`-only (the 0.7.2 attribute-macro support was reverted in 0.8.0).
    // New bare `impl template {body}` syntax → uniformly rewritten to legacy
    // `impl{template}` (the same collection as bare `where`): a bare `impl`
    // not followed by `{...}` collects its template fragment up to the shared
    // boundary (`impl (A@..) {body}` ≡ `impl{(A@..)} {body}`; a second bare
    // `impl` starts a new template — `impl A<B> impl @{}` ≡
    // `impl{A<B>} impl{@{}}`, merged like adjacent `where` regions). Runs
    // **before** `mark_varseg`: a bare `impl (A@..)` fragment's `ident@..`
    // must land inside an `impl{...}` template group before the variadic
    // marker pass scans for it.
    // ---- preprocessing (typestate pipeline, see `preprocess/stream.rs`):
    // `Raw → Paired → DirectivesResolved → WhereDone → Ready`. The
    // bare-`impl` collection → variadic marking → `@` expansion → pairing
    // prefix is the shared `preprocess()`; the attribute tail expands `#`
    // directives, rewrites bare `where`, and copies `A<>`. The order is
    // enforced by the stream's states (the `Foo<>`-in-predicates passthrough
    // is why `where_process` precedes `expand_empty_trait_generics`).
    let is_unsafe = trait_item.unsafety.is_some();
    let trait_bounds = crate::analyze::extract_trait_bounds(&trait_item);
    let ready = crate::preprocess::stream::new(rest_tokens)
        .preprocess(ConstCtx::Attribute {
            trait_def: &trait_item,
            trait_full_path: &trait_full_path,
        })?
        .expand_tokens(&trait_item, &trait_full_path)?
        .where_process()?
        .expand_empty_trait_generics(&trait_item, &trait_bounds)?;
    let expanded = ready.into_tokens();
    // Trait generic param names — needed by the codegen postprocess (trait
    // generic substitution) for *both* entry macros: batch_impl_only drops the
    // trait definition but still substitutes its params in directive bodies.
    // Only type/const params are collected — lifetime params stay untouched:
    // a body's `'a` refers to the impl's own lifetime generic, not to a
    // substituted trait arg.
    let trait_param_names = trait_item
        .generics
        .params
        .iter()
        .filter_map(|p| match p {
            syn::GenericParam::Type(tp) => Some(tp.ident.clone()),
            syn::GenericParam::Const(cp) => Some(cp.ident.clone()),
            syn::GenericParam::Lifetime(_) => None,
        })
        .collect::<Vec<_>>();
    let start_trait = if include_trait { trait_item.into() } else { None };
    Ok(PreparedAttr {
        trait_full_path,
        trait_last_ident,
        is_unsafe,
        start_trait,
        trait_bounds,
        trait_param_names,
        expanded,
    })
}

/// Segment-level `@trait` → this segment's full trait path (batch_trait!-specific; constant
/// values like `<T>@trait<T>` keep `@trait` via lazy expansion, replaced here per segment —
/// each segment uses its own name).
fn replace_segment_trait(
    tokens: Vec<TokenTree>, trait_full_path: &TokenStream,
) -> Result<Vec<TokenTree>, TokenStream> {
    let mut out = vec![];
    let mut i = 0;
    while let Some(cur) = tokens.get(i) {
        if let TokenTree::Punct(p) = cur
            && p.as_char() == '@'
            && let Some(TokenTree::Ident(id)) = tokens.get(i + 1)
            && id == "trait"
        {
            out.extend(trait_full_path.clone());
            i += 2;
        } else if let TokenTree::Group(g) = cur {
            // Recurse into groups (where{...} predicates and type groups):
            // segment-level `@trait` must reach every DSL structure, not
            // just the top level.
            let inner = g.stream().into_iter().collect::<Vec<_>>();
            let inner = replace_segment_trait(inner, trait_full_path)?;
            out.push(proc_macro2::Group::new(g.delimiter(), inner.into_iter().collect()).into());
            i += 1;
        } else {
            out.push(cur.clone());
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
    // ---- preprocessing (typestate pipeline): `Raw → Paired → WhereDone`.
    // `batch_trait!` has no trait definition, so there is no `#` expansion
    // (`expand_tokens` needs the trait) and no `A<>` copy — the tail is just
    // the bare-`where` rewrite; the segment loop below handles `@trait` per
    // segment. The `@`-before-pairing reason lives in `preprocess()`.
    let where_done = crate::preprocess::stream::new(tokens)
        .preprocess(crate::preprocess::ConstCtx::Trait { user_table: &user_consts })?
        .where_process()?;
    let tokens = where_done.into_tokens();
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
        let is_unsafe = matches!(cursor.peek(), Some(TokenTree::Ident(id)) if *id == "unsafe");
        if is_unsafe {
            cursor.bump();
        }

        // Collect the trait path (stop at `:`; collect `::` path separators too).
        // Angle brackets were paired into opaque groups by angle_collect, so no `<>` depth tracking.
        let path_start = cursor.pos();
        while let Some(token) = cursor.peek() {
            match token {
                TokenTree::Punct(p) if p.as_char() == ':' => {
                    if cursor.is_single_colon() {
                        break;
                    } else {
                        // `::` — consume both colons; `advance` clamps, so a
                        // dangling joint `:` cannot move the cursor past the
                        // end (the `slice_since` below relies on `pos <= len`).
                        cursor.advance(2);
                    }
                }
                _ => cursor.bump(),
            }
        }
        let trait_path = cursor.slice_since(path_start);
        if trait_path.is_empty() {
            return Err(compile_error_str(
                "batch-impl: batch_trait! expects a trait name",
                cursor.span(),
            ));
        }
        // Full trait path: just collect the token stream of trait_path as-is
        let trait_full_path = trait_path.iter().cloned().collect();
        // Take the last ident in the path as the `trait_name` used for matching
        let Some(trait_last_ident) = trait_path
            .iter()
            .filter_map(|tt| if let TokenTree::Ident(id) = tt { id.into() } else { None })
            .next_back()
        else {
            return Err(compile_error_str(
                "batch-impl: batch_trait! expects an ident as the trait name",
                trait_path.first().map_or_else(proc_macro2::Span::call_site, |t| t.span()),
            ));
        };
        if !cursor.is_punct(':') {
            return Err(compile_error_str(
                "batch-impl: batch_trait! expects ':' to separate the trait name and impl-specs",
                cursor.span(),
            ));
        }
        cursor.bump();
        // Segment boundary = first depth-0 `;` (not consumed; skipped by the loop head)
        let spec = cursor.take_segment(&[';']).to_vec();
        // The span of this segment's own tokens: rustc blames the **invocation** for an error inside a
        // generated item, so a `batch_trait!` holding 20 specs reported every such error on the
        // macro's first line (alga2's report 3: "the real culprit is at line 138").
        let segment_span =
            trait_path.first().map_or_else(proc_macro2::Span::call_site, |t| t.span());
        // Segment-level `@trait` replacement: batch_trait!'s `@trait` is kept as-is during
        // the constant stage (each segment has a different trait name), expanded here to
        // this segment's full trait path — the `@type_t=<T>@trait<T>` cross-segment reuse
        // scenario (`A: @type_t ...` / `B: @type_t ...`).
        let spec = replace_segment_trait(spec, &trait_full_path)?;
        result.extend(respan_call_site(
            run_pipeline(
                &spec,
                Op::Comma,
                &trait_full_path,
                trait_last_ident,
                is_unsafe,
                None,
                // batch_trait! has no trait definition, so generic bounds cannot be inherited
                &Default::default(),
                &[],
            )?,
            segment_span,
        ));
    }
    Ok(result.into())
}

/// Give every token that still points at the macro **call site** the span of the segment it came from.
///
/// rustc attributes an error inside a *generated* item to the macro invocation, so a long
/// `batch_trait!` reported every spec-level error on the macro's opening line. Only call-site tokens
/// are re-spanned: a token that already carries a span from the user's own input keeps it, so the
/// precision the diagnostics already had is untouched (`A<u8>` on a non-generic `A` points at that
/// segment by itself; an `E0046` on a generated impl does not).
fn respan_call_site(tokens: TokenStream, span: proc_macro2::Span) -> TokenStream {
    fn is_call_site(candidate: proc_macro2::Span) -> bool {
        // `proc_macro2::Span` has no `PartialEq` (the trait method `eq` resolves to `Iterator::eq` and
        // does not compile), so the two spans are compared through their `Debug` form: a synthetic
        // call-site span carries no file/line, a span from the user's source does.
        format!("{candidate:?}") == format!("{:?}", proc_macro2::Span::call_site())
    }
    tokens
        .into_iter()
        .map(|tt| {
            if is_call_site(tt.span()) {
                let mut out = tt;
                out.set_span(span);
                out
            } else {
                tt
            }
        })
        .collect()
}
