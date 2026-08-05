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
    match ty {
        Ty::WithType(wt) => {
            let mut parts = extract_impl_parts(*wt.1);
            let (impl_generics, associated_types) =
                (parts.impl_generics, parts.associated_types);
            parts.impl_generics = wt.0.params;
            parts.associated_types = wt.0.bindings;
            parts.impl_generics.extend(impl_generics);
            parts.associated_types.extend(associated_types);
            parts
        }
        Ty::WithTrait(wt) => {
            let mut parts = extract_impl_parts(*wt.1);
            parts.trait_generic_names.extend(wt.0.1.params.into_iter().map(|p| p.0));
            parts.associated_types.extend(wt.0.1.bindings);
            parts
        }
        Ty::WithCode(wc) => match wc.0 {
            Some(inner) => {
                let mut parts = extract_impl_parts(*inner);
                match &mut parts.body {
                    Some(t) => t.extend(wc.1.0),
                    None => parts.body = wc.1.0.into(),
                }
                parts
            }
            // bare code block has no target type; defensive fallback
            None => ImplParts::leaf(wc.into()),
        },
        Ty::WithWhere(ww) => match ww.0 {
            Some(inner) => {
                let mut parts = extract_impl_parts(*inner);
                parts.where_clauses.push(ww.1.0);
                parts
            }
            None => ImplParts::leaf(ww.into()),
        },
        Ty::WithAttr(wa) => match wa.1 {
            Some(inner) => {
                let mut parts = extract_impl_parts(*inner);
                let stream = &wa.0.0;
                parts.attrs.push(quote!(#[#stream]));
                parts
            }
            None => ImplParts::leaf(wa.into()),
        },
        Ty::WithPrefix(wp) => match wp.1 {
            Some(inner) => {
                let mut parts = extract_impl_parts(*inner);
                match wp.0 {
                    // unsafe prefix → mark as unsafe impl
                    TyPrefix::Unsafe => parts.is_unsafe_impl = true,
                    // reference/pointer prefix → wrap onto the target type
                    _ => {
                        parts.target_type =
                            TyWithPrefix(wp.0, parts.target_type.into()).into()
                    }
                }
                parts
            }
            None => ImplParts::leaf(wp.into()),
        },
        Ty::Error(e) => ImplParts::leaf(e.into()),
        o => ImplParts::leaf(o),
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
    match ty {
        Ty::WithType(wt) => {
            out.extend(wt.0.params);
            hoist_type_params(*wt.1, out)
        }
        Ty::Array(a) => {
            TyArray(a.0.into_iter().map(|e| hoist_type_params(e, out)).collect())
                .into()
        }
        Ty::Tuple(t) => {
            TyTuple(t.0.into_iter().map(|e| hoist_type_params(e, out)).collect())
                .into()
        }
        Ty::Group(g) => TyGroup(hoist_type_params(*g.0, out).into()).into(),
        Ty::PrimitiveArray(pa) => {
            TyPrimitiveArray(pa.0.map(|e| hoist_type_params(*e, out).into()), pa.1)
                .into()
        }
        Ty::Generic(g) => {
            let base = hoist_type_params(*g.0, out);
            TyGeneric(base.into(), g.1).into()
        }
        Ty::WithPrefix(wp) => {
            TyWithPrefix(wp.0, wp.1.map(|e| hoist_type_params(*e, out).into())).into()
        }
        Ty::WithTrait(wt) => {
            TyWithTrait(wt.0, hoist_type_params(*wt.1, out).into()).into()
        }
        Ty::WithCode(wc) => {
            TyWithCode(wc.0.map(|e| hoist_type_params(*e, out).into()), wc.1).into()
        }
        Ty::WithWhere(ww) => {
            TyWithWhere(ww.0.map(|e| hoist_type_params(*e, out).into()), ww.1).into()
        }
        Ty::WithAttr(wa) => {
            TyWithAttr(wa.0, wa.1.map(|e| hoist_type_params(*e, out).into())).into()
        }
        Ty::Fn(f) => TyFn(
            f.0.map(|params| {
                params.into_iter().map(|p| hoist_type_params(p, out)).collect()
            }),
            f.1.map(|r| hoist_type_params(*r, out).into()),
            f.2,
        )
        .into(),
        other => other,
    }
}
