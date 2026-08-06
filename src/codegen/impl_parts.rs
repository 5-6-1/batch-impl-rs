//! Impl metadata extraction: recursively pulls out the parts an impl block needs
//! (generics / bindings / attrs / unsafe).

use proc_macro2::TokenStream;
use quote::quote;

use crate::ast::*;

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
            let (impl_generics, associated_types) =
                (parts.impl_generics, parts.associated_types);
            parts.impl_generics = wt.0.params;
            parts.associated_types = wt.0.bindings;
            parts.impl_generics.extend(impl_generics);
            parts.associated_types.extend(associated_types);
            parts
        }
        TyKind::WithTrait(wt) => {
            let mut parts = extract_impl_parts(*wt.1);
            parts.trait_generic_names.extend(wt.0.1.params.into_iter().map(|p| p.0));
            parts.associated_types.extend(wt.0.1.bindings);
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
                parts.where_clauses.push(ww.1.0);
                parts
            }
            None => ImplParts::leaf(ww.to_ty().with_span(span)),
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
                        parts.target_type = Ty::new(
                            span,
                            TyWithPrefix(wp.0, old_target.into()).into(),
                        );
                    }
                }
                parts
            }
            None => ImplParts::leaf(Ty::new(span, TyKind::WithPrefix(wp))),
        },
        TyKind::Error(e) => ImplParts::leaf(Ty::new(span, TyKind::Error(e))),
        o => ImplParts::leaf(Ty::new(span, o)),
    }
}

/// Recursively hoists nested `WithType` generic declarations in a type (e.g. the
/// fresh-generic tuple of `()^N`).
///
/// Collects the params of `WithType(<A>, T)` into `out` (for the impl generics) and
/// replaces that node with its inner `T`. Must recurse into every container (Array /
/// Tuple / Group / PrimitiveArray / Generic / WithPrefix / WithTrait / WithCode /
/// WithWhere / WithAttr / Fn).
pub(crate) fn hoist_type_params(
    ty: Ty, out: &mut Vec<(TokenStream, Option<Ty>)>,
) -> Ty {
    match ty.kind {
        // Generic-declaration wrapper: hoist the declaration outward (params
        // are added to `out`, not to the rebuilt node).
        TyKind::WithType(wt) => {
            out.extend(wt.0.params);
            hoist_type_params(*wt.1, out)
        }
        // All other variants: recurse into children uniformly.
        other => {
            Ty::new(ty.span, other).map_children(&mut |c| hoist_type_params(c, out))
        }
    }
}
