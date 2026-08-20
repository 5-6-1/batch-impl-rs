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

use crate::ast::{Op, Ty};
use crate::codegen::{Mapping, apply_mapping, match_shape};
use crate::entry::driver::collect_spec_leaves;
use crate::parse::split_at_depth0;
use crate::preprocess::{angle_collect, render_angles, where_process};
use crate::util::{Cursor, compile_error_str, is_single_colon};

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

    // ---- preprocessing subset (todos impl entry §G) ----
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
    let (spec, where_preds) = peel_where(spec);
    match find_shape_colon(spec) {
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
            // Shape-validity check (todos §I20): the impl's for-Type must
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
            let (new_gen, matrix) = split_new_gen(&spec[colon + 1..]);
            if matrix.is_empty() {
                // Empty matrix source → N = 1, the shape itself (no slot
                // mapping; the for-Type is emitted verbatim).
                return assemble_impl(
                    item,
                    trait_path,
                    new_gen.as_ref(),
                    &where_preds,
                    &Mapping::default(),
                    item.self_ty.to_token_stream(),
                );
            }
            let leaves = parse_matrix_leaves(&matrix)?;
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
                out.extend(assemble_impl(
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
            let (new_gen, for_tokens) = split_new_gen(spec);
            let for_tokens = render_angles(for_tokens.iter().cloned().collect::<TokenStream>());
            let _for_ty: syn::Type = syn::parse2(for_tokens.clone()).map_err(|_| {
                compile_error_str(
                    "batch-impl: the direct form needs a standard Rust type after \
                     the generic declaration (e.g. `<T> Box<T>`)",
                    Span::call_site(),
                )
            })?;
            assemble_impl(
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

/// Assembles one generated impl: generics (attr new-generic-decl first, then
/// the impl's own params), trait path, rewritten for-Type, merged where
/// clause, rewritten body. `m` is the slot mapping (empty for the direct
/// form / empty matrix).
#[allow(clippy::too_many_arguments)]
fn assemble_impl(
    item: &ItemImpl, trait_path: &syn::Path, new_gen: Option<&TokenStream>,
    where_preds: &[TokenTree], m: &Mapping, for_ty: TokenStream,
) -> Result<TokenStream, TokenStream> {
    let entries = m.entries();
    let item_params = item.generics.params.iter().map(|p| p.to_token_stream()).collect::<Vec<_>>();
    // Generics: the attr new-generic-decl first, then the impl's own params.
    let gen_tokens = match new_gen {
        Some(ng) => {
            let ng_empty = ng.clone().into_iter().next().is_none();
            match (ng_empty, item_params.is_empty()) {
                (true, true) => quote!(),
                (true, false) => quote!(<#(#item_params),*>),
                (false, true) => quote!(<#ng>),
                (false, false) => quote!(<#ng, #(#item_params),*>),
            }
        }
        None => {
            if item_params.is_empty() {
                quote!()
            } else {
                quote!(<#(#item_params),*>)
            }
        }
    };
    let mut preds = vec![];
    if !where_preds.is_empty() {
        preds.push(apply_mapping(where_preds.iter().cloned().collect(), entries));
    }
    if let Some(wc) = &item.generics.where_clause {
        preds.push(apply_mapping(wc.predicates.to_token_stream(), entries));
    }
    let where_clause = if preds.is_empty() { quote!() } else { quote!(where #(#preds),*) };
    let items = item
        .items
        .iter()
        .map(|it| apply_mapping(it.to_token_stream(), entries))
        .collect::<Vec<_>>();
    let unsafe_kw = if item.unsafety.is_some() { quote!(unsafe) } else { quote!() };
    Ok(quote! {
        #unsafe_kw impl #gen_tokens #trait_path for #for_ty #where_clause {
            #(#items)*
        }
    })
}

/// Parses a matrix-source (DSL expression) into its leaf types.
fn parse_matrix_leaves(matrix: &[TokenTree]) -> Result<Vec<Ty>, TokenStream> {
    let mut cursor = Cursor::new(matrix);
    let (leaves, errors) = collect_spec_leaves(&mut cursor, Op::Comma, None);
    if !errors.is_empty() {
        return Err(errors.into_iter().collect());
    }
    Ok(leaves)
}

/// `where{...}` tail (the where_process output shape) → (spec without the
/// where, predicate tokens).
fn peel_where(spec: &[TokenTree]) -> (&[TokenTree], Vec<TokenTree>) {
    if spec.len() >= 2
        && let Some(TokenTree::Group(g)) = spec.last()
        && g.delimiter() == proc_macro2::Delimiter::Brace
        && let Some(TokenTree::Ident(w)) = spec.get(spec.len() - 2)
        && *w == "where"
    {
        (&spec[..spec.len() - 2], g.stream().into_iter().collect())
    } else {
        (spec, vec![])
    }
}

/// The depth-0 single `:` that separates the shape template from the rest.
fn find_shape_colon(spec: &[TokenTree]) -> Option<usize> {
    spec.iter().enumerate().find_map(|(i, tt)| {
        matches!(tt, TokenTree::Punct(_) if is_single_colon(spec, i)).then_some(i)
    })
}

/// `new-generic-decl?` at the head: a `delimiter![<>]` group. Returns (decl
/// contents, rest).
fn split_new_gen(tokens: &[TokenTree]) -> (Option<TokenStream>, Vec<TokenTree>) {
    match tokens.first() {
        Some(TokenTree::Group(g)) if g.delimiter() == delimiter![<>] => {
            (Some(g.stream()), tokens[1..].to_vec())
        }
        _ => (None, tokens.to_vec()),
    }
}

/// `@trait` → the impl's trait path; every other `@` construct and every `#`
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
