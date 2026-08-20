//! The impl entry of `#[batch_impl]` — batch-instantiate a
//! hand-written `impl` block from a shape-template × matrix-source
//! description.
//!
//! `#[batch_impl(A<B> : [Box,Rc].[usize,isize])] impl Tr for A<B> {...}`
//! emits one impl per matrix leaf (`Box<usize>` / `Box<isize>` / `Rc<usize>` /
//! `Rc<isize>`): the shape template is matched against each leaf by the
//! shared `codegen::shape` kernel, and the slot mapping rewrites the
//! for-Type / where predicates / body. The original impl is withheld (its
//! for-Type holds the placeholder slot names).
//!
//! The impl block itself is **ordinary Rust** (`impl Tr<...> for T { ... }`
//! must parse as `syn::ItemImpl` verbatim) — the DSL lives only in the
//! attribute. The body / for-Type therefore stay standard Rust: no
//! variadic segments, no repeat blocks, no DSL operators. `X<>` (empty
//! brackets) in the **where predicates** fills with the impl's trait args
//! (`impl Tr<Additive, Multiplicative> for ...` → `Marker<>` =
//! `Marker<Additive, Multiplicative>`), the same sync as the trait entries.
//!
//! Attr grammar (single-spec common case; `;` separates multiple specs):
//! - shape form: `shape-template : new-generic-decl? matrix-source? (where ...)?`
//! - direct form: `new-generic-decl? for-type (where ...)?`
//!
//! `@trait` (→ the impl's trait path) is allowed in new-generic-decl bounds
//! and where predicates; every other `@` construct and every `#` directive
//! is rejected on this entry.

use proc_macro2::{Group, Span, TokenStream, TokenTree};
use quote::{ToTokens, quote};
use syn::ItemImpl;

use crate::codegen::{Mapping, apply_mapping, match_shape};
use crate::parse::split_at_depth0;
use crate::preprocess::{angle_collect, render_angles, where_process};
use crate::util::compile_error_str;

/// Entry: expand `#[batch_impl(<dsl>)] impl ...` into N `impl` blocks.
pub(crate) fn expand_impl_entry(
    attr: TokenStream, item: ItemImpl,
) -> Result<TokenStream, TokenStream> {
    let trait_path = item.trait_.as_ref().map(|(path, _)| path.clone()).ok_or_else(|| {
        compile_error_str(
            "batch-impl: the annotated item must be a trait impl (`impl Trait for Type`)",
            Span::call_site(),
        )
    })?;

    // ---- preprocessing subset: angle pairing → `@trait` replacement →
    // bare-`where` rewrite (see the entry module docs) ----
    let attr_vec = attr.into_iter().collect::<Vec<_>>();
    let paired = angle_collect(&attr_vec)?;
    let paired = replace_trait_at(&paired, &trait_path)?;
    // The ItemImpl attr has no body after the predicates, so the end of the
    // stream terminates the where region (`allow_end = true`).
    let paired = where_process(&paired, true)?;

    // ---- `;`-separated specs (the single-spec case is the common one) ----
    let mut out = quote![];
    for spec in split_at_depth0(&paired, ';') {
        if spec.is_empty() {
            continue;
        }
        out.extend(expand_one_spec(spec, &item, &trait_path)?);
    }
    Ok(render_angles(out))
}

/// Expands one spec (shape form or direct form) into its impl(s).
fn expand_one_spec(
    spec: &[TokenTree], item: &ItemImpl, trait_path: &syn::Path,
) -> Result<TokenStream, TokenStream> {
    // `where{...}` (where_process output) is the tail.
    let (spec, where_preds) = crate::entry::impl_spec::peel_where(spec);
    match crate::entry::impl_spec::find_shape_colon(spec) {
        Some(colon) => {
            // ---- shape form: `shape-template : new-generic-decl? matrix-source?` ----
            // The angle groups must be restored to flat `<...>` before syn
            // parsing (render_angles; syn cannot consume the
            // `delimiter![<>]` carrier groups).
            let template_tokens =
                render_angles(spec[..colon].iter().cloned().collect::<TokenStream>());
            let template: syn::Type =
                syn::parse2(template_tokens).map_err(|e| {
                    compile_error_str(
                        &format!(
                            "batch-impl: the shape template before `:` is not a valid type ({e})",
                        ),
                        Span::call_site(),
                    )
                })?;
            // Shape-validity check: the impl's for-Type must
            // match the template ident-for-ident (zero bindings) — a binding
            // means the for-Type doesn't carry the placeholder slot names.
            let for_type: syn::Type =
                syn::parse2(item.self_ty.to_token_stream()).map_err(|_| {
                    compile_error_str(
                        "batch-impl: the impl's for-Type is not a valid type",
                        Span::call_site(),
                    )
                })?;
            let check = match_shape(&template, &for_type)
                .map(|(m, _)| m)
                .map_err(|e| compile_error_str(&e.message(), Span::call_site()))?;
            if !check.entries().is_empty() {
                return Err(compile_error_str(
                    "batch-impl: the impl's for-Type must match the shape template \
                     ident-for-ident (write the same placeholder names, e.g. `impl Tr for A<B>` \
                     with template `A<B>`)",
                    Span::call_site(),
                ));
            }
            let (new_gen, matrix) = crate::entry::impl_spec::split_new_gen(&spec[colon + 1..]);
            if matrix.is_empty() {
                // Empty matrix source → N = 1, the shape itself (no slot
                // mapping; the for-Type is emitted verbatim).
                return crate::entry::impl_spec::assemble_impl(
                    item,
                    trait_path,
                    new_gen.as_ref(),
                    &where_preds,
                    &Mapping::default(),
                    item.self_ty.to_token_stream(),
                );
            }
            let leaves = crate::entry::impl_spec::parse_matrix_leaves(&matrix)?;
            let mut out = quote![];
            for leaf in leaves {
                let leaf_tokens = leaf.to_token_stream();
                let leaf_ty: syn::Type = syn::parse2(leaf_tokens.clone()).map_err(|_| {
                    compile_error_str(
                        "batch-impl: the matrix leaf is not a standard Rust type \
                         (generators cannot be destructured by a shape template)",
                        Span::call_site(),
                    )
                })?;
                let m = match_shape(&template, &leaf_ty)
                    .map(|(m, _)| m)
                    .map_err(|e| compile_error_str(&e.message(), Span::call_site()))?;
                // for-Type: slot names rewritten to the bound leaf subtrees.
                let for_ty = apply_mapping(item.self_ty.to_token_stream(), m.entries());
                out.extend(crate::entry::impl_spec::assemble_impl(
                    item,
                    trait_path,
                    new_gen.as_ref(),
                    &where_preds,
                    &m,
                    for_ty,
                )?);
            }
            Ok(out)
        }
        None => {
            // ---- direct form: `new-generic-decl? for-type` (no matrix, N = 1) ----
            let (new_gen, for_tokens) = crate::entry::impl_spec::split_new_gen(spec);
            let for_tokens = render_angles(for_tokens.iter().cloned().collect::<TokenStream>());
            let _for_ty: syn::Type = syn::parse2(for_tokens.clone()).map_err(|_| {
                compile_error_str(
                    "batch-impl: the direct form needs a standard Rust type after \
                     the generic declaration (e.g. `<T> Box<T>`)",
                    Span::call_site(),
                )
            })?;
            crate::entry::impl_spec::assemble_impl(
                item,
                trait_path,
                new_gen.as_ref(),
                &where_preds,
                &Mapping::default(),
                for_tokens,
            )
        }
    }
}

/// directive is rejected (custom constants / selectors / position refs are
/// all banned on this entry). `#[...]` attributes pass through.
fn replace_trait_at(
    tokens: &[TokenTree], trait_path: &syn::Path,
) -> Result<Vec<TokenTree>, TokenStream> {
    let mut out = vec![];
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            TokenTree::Punct(p) if p.as_char() == '@' => match tokens.get(i + 1) {
                Some(TokenTree::Ident(id)) if id == "trait" => {
                    if matches!(tokens.get(i + 2), Some(TokenTree::Punct(p2)) if p2.as_char() == '<')
                    {
                        return Err(compile_error_str(
                            "batch-impl: `@trait<...>` is not supported on the ItemImpl entry \
                             (write the trait args directly)",
                            tokens[i].span(),
                        ));
                    }
                    out.extend(quote!(#trait_path));
                    i += 2;
                }
                Some(TokenTree::Ident(_)) => {
                    return Err(compile_error_str(
                        "batch-impl: only `@trait` is allowed on the ItemImpl entry \
                         (`@` constants are not supported)",
                        tokens[i].span(),
                    ));
                }
                Some(TokenTree::Literal(_)) => {
                    return Err(compile_error_str(
                        "batch-impl: `@N` / `@g_i` position references are not supported \
                         on the ItemImpl entry",
                        tokens[i].span(),
                    ));
                }
                _ => {
                    return Err(compile_error_str(
                        "batch-impl: `@` must be followed by `trait` on the ItemImpl entry",
                        tokens[i].span(),
                    ));
                }
            },
            // `#` directives are banned; `#[...]` attributes pass through.
            TokenTree::Punct(p) if p.as_char() == '#' => {
                if matches!(tokens.get(i + 1), Some(TokenTree::Group(g))
                    if g.delimiter() == proc_macro2::Delimiter::Bracket)
                {
                    out.push(tokens[i].clone());
                    out.push(tokens[i + 1].clone());
                    i += 2;
                } else {
                    return Err(compile_error_str(
                        "batch-impl: `#` directives are not supported on the ItemImpl entry \
                         (write the impl body directly)",
                        tokens[i].span(),
                    ));
                }
            }
            TokenTree::Group(g) => {
                let inner =
                    replace_trait_at(&g.stream().into_iter().collect::<Vec<_>>(), trait_path)?;
                let mut ng = Group::new(g.delimiter(), inner.into_iter().collect());
                ng.set_span(g.span());
                out.push(TokenTree::Group(ng));
                i += 1;
            }
            _ => {
                out.push(tokens[i].clone());
                i += 1;
            }
        }
    }
    Ok(out)
}
