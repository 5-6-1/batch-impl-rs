//! Impl metadata extraction: recursively pulls out the parts an impl block needs
//! (generics / bindings / attrs / unsafe).

use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use std::collections::HashSet;

use crate::TraitBounds;
use crate::ast::*;
use crate::parse::split_at_depth0;
use crate::util::compile_err;

/// Recursively extracts all parts an impl block needs from `Ty`.
///
/// The `impl_spec` AST nodes nest in modifier order (e.g. `<T> Trait<T> unsafe Box<T> { body }`);
/// this function recursively dismantles the tree, collecting: impl generics, trait generics,
/// associated type bindings, target type, body, attrs, unsafe flag.
pub(crate) struct ImplParts {
    pub(crate) impl_generics: Vec<(TokenStream, Option<Ty>)>,
    pub(crate) trait_generic_names: Vec<TokenStream>,
    pub(crate) associated_types: Vec<(TokenStream, TokenStream)>,
    pub(crate) target_type: Ty,
    pub(crate) body: Option<TokenStream>,
    pub(crate) attrs: Vec<TokenStream>,
    pub(crate) is_unsafe_impl: bool,
    /// where predicates from a `where{...}` suffix; multiple ones are joined into
    /// `where P1, P2, ...`, elements connected by commas.
    pub(crate) where_clauses: Vec<TokenStream>,
    /// `impl{...}` Self-part shape templates (Ext 2), in attachment order —
    /// matched against the leaf target type by `codegen::shape::match_shape`,
    /// the merged slot mapping rewrites the target/where/body.
    pub(crate) impl_templates: Vec<TokenStream>,
}

impl ImplParts {
    /// leaf node: no modifiers, just the target type
    fn leaf(target_type: Ty) -> Self {
        ImplParts {
            impl_generics: vec![],
            trait_generic_names: vec![],
            associated_types: vec![],
            target_type,
            body: None,
            attrs: vec![],
            is_unsafe_impl: false,
            where_clauses: vec![],
            impl_templates: vec![],
        }
    }
}

/// Recursively dismantles the `Ty` tree, extracting all metadata an impl block needs.
///
/// Each wrapper node encountered strips its contribution (generics, bindings, attrs,
/// unsafe) and recurses into the inner, until a leaf node (pure target type; bare
/// wrappers with `None` inner and `Ty::Error` also fall through to `leaf` defensively).
pub(crate) fn extract_impl_parts(ty: Ty) -> ImplParts {
    let Ty { span, kind } = ty;
    match kind {
        TyKind::WithType(wt) => {
            let mut parts = extract_impl_parts(*wt.1);
            let (impl_generics, associated_types) = (parts.impl_generics, parts.associated_types);
            parts.impl_generics =
                wt.0.params.into_iter().map(|(n, b)| (n.to_token_stream(), b)).collect();
            parts.associated_types =
                wt.0.bindings
                    .into_iter()
                    .map(|(n, v)| (n.to_token_stream(), v.to_token_stream()))
                    .collect();
            parts.impl_generics.extend(impl_generics);
            parts.associated_types.extend(associated_types);
            parts
        }
        TyKind::WithTrait(wt) => {
            let mut parts = extract_impl_parts(*wt.1);
            // Trait generic args may carry splats (`Conv<*(A,B)>`) and
            // generators (`Conv<()^2>`) — flatten them before rendering
            // (token-level: `trait_generic_names` is `TokenStream` past this
            // point). A hoisted fresh declaration joins the impl generics
            // (the names it carries must be declared for the impl to
            // compile) — the same rule as the generic-arg position.
            let (flat, decl) = flat_splat_params(wt.0.1.params);
            parts.trait_generic_names.extend(flat.into_iter().map(|(n, _)| n.to_token_stream()));
            if let Some(d) = decl {
                parts
                    .impl_generics
                    .extend(d.params.into_iter().map(|(n, b)| (n.to_token_stream(), b)));
            }
            parts.associated_types.extend(
                wt.0.1
                    .bindings
                    .into_iter()
                    .map(|(n, v)| (n.to_token_stream(), v.to_token_stream())),
            );
            parts
        }
        TyKind::WithCode(wc) => match wc.0 {
            Some(inner) => {
                let mut parts = extract_impl_parts(*inner);
                match &mut parts.body {
                    Some(t) => t.extend(wc.1.0),
                    None => parts.body = wc.1.0.into(),
                }
                parts
            }
            // bare code block has no target type; defensive fallback
            None => ImplParts::leaf(wc.to_ty().with_span(span)),
        },
        TyKind::WithWhere(ww) => match ww.0 {
            Some(inner) => {
                let mut parts = extract_impl_parts(*inner);
                // Split the where group into predicates at depth-0 commas so
                // each predicate resolves independently (`@all_fresh` /
                // `@N..M` expansions must not swallow following predicates).
                let tokens = ww.1.0.clone().into_iter().collect::<Vec<_>>();
                for pred in split_at_depth0(&tokens, ',') {
                    parts.where_clauses.push(pred.iter().cloned().collect());
                }
                parts
            }
            None => ImplParts::leaf(ww.to_ty().with_span(span)),
        },
        TyKind::WithImpl(wi) => match wi.0 {
            Some(inner) => {
                let mut parts = extract_impl_parts(*inner);
                // The template is consumed by the codegen shape match (never
                // emitted); multiple `impl{...}` attachments merge into one
                // mapping (redundant identical bindings legal, conflicting
                // ones error).
                parts.impl_templates.push(wi.1.0);
                parts
            }
            None => ImplParts::leaf(wi.to_ty().with_span(span)),
        },
        TyKind::WithAttr(wa) => match wa.1 {
            Some(inner) => {
                let mut parts = extract_impl_parts(*inner);
                let stream = &wa.0.0;
                parts.attrs.push(quote!(#[#stream]));
                parts
            }
            None => ImplParts::leaf(wa.to_ty().with_span(span)),
        },
        TyKind::WithPrefix(wp) => match wp.1 {
            Some(inner) => {
                let mut parts = extract_impl_parts(*inner);
                match wp.0 {
                    // unsafe prefix → mark as unsafe impl
                    TyPrefix::Unsafe => parts.is_unsafe_impl = true,
                    // reference/pointer prefix → wrap onto the target type
                    _ => {
                        let old_target = std::mem::replace(
                            &mut parts.target_type,
                            TyWithPrefix(wp.0, None).to_ty().with_span(span),
                        );
                        parts.target_type =
                            TyWithPrefix(wp.0, old_target.into()).to_ty().with_span(span);
                    }
                }
                parts
            }
            None => ImplParts::leaf(wp.to_ty().with_span(span)),
        },
        TyKind::Error(e) => ImplParts::leaf(Ty { span, kind: TyKind::Error(e) }),
        o => ImplParts::leaf(Ty { span, kind: o }),
    }
}

/// Recursively hoists nested `WithType` generic declarations in a type (e.g. the
/// fresh-generic tuple of `()^N`).
///
/// Collects the params of `WithType(<A>, T)` into `out` (for the impl generics) and
/// replaces that node with its inner `T`. Must recurse into every container (Array /
/// Tuple / Group / PrimitiveArray / Generic / WithPrefix / WithTrait / WithCode /
/// WithWhere / WithAttr / Fn).
pub(crate) fn hoist_type_params(ty: Ty, out: &mut Vec<(TokenStream, Option<Ty>)>) -> Ty {
    match ty.kind {
        // Generic-declaration wrapper: hoist the declaration outward (params
        // are added to `out`, not to the rebuilt node). Same-named fresh
        // params are collected once — `(T,)^N` clones `T` (a generator such
        // as `()^3`) N times, and each clone carries its own declaration of
        // the same fresh names; the clones reference one shared generic.
        TyKind::WithType(wt) => {
            for (name, bound) in wt.0.params {
                let name_str = name.to_token_stream().to_string();
                if is_fresh_name(&name_str)
                    && let Some(existing) = out.iter_mut().find(|(n, _)| n.to_string() == name_str)
                {
                    // Prefer a declaration with a bound over the bare one.
                    if existing.1.is_none() {
                        existing.1 = bound;
                    }
                } else {
                    out.push((name.to_token_stream(), bound));
                }
            }
            hoist_type_params(*wt.1, out)
        }
        // All other variants: recurse into children uniformly.
        other => Ty { span: ty.span, kind: other }.map_children(&mut |c| hoist_type_params(c, out)),
    }
}

/// Inherits trait generic bounds onto impl generic params **without a written
/// bound** (same-name inheritance, positional match) and appends the trait's
/// unmerged where predicates to the impl (after a reference check). Returns
/// the collected errors; on any error the caller emits only the errors — no
/// partial impl. Rules: see the `TraitBounds` docs.
pub(crate) fn inherit_trait_bounds(
    parts: &mut ImplParts, trait_bounds: &TraitBounds, trait_args: &[String],
    impl_names: &HashSet<String>,
) -> Vec<TokenStream> {
    let mut errs = vec![];
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
    errs
}
