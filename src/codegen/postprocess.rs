//! Codegen postprocess: transformations over `ImplParts` after extraction.
//! Trait generic substitution (`From<bool>`: `value: T` → `value: bool` in
//! directive-copied bodies) lives here — `ImplParts` carries both the trait
//! arg names (`trait_generic_names`) and the full body (fn signature + user
//! code block), so the substitution needs no plumbing through preprocess.

use proc_macro2::{Ident, TokenStream, TokenTree};

use crate::ast::*;
use crate::codegen::impl_parts::ImplParts;

/// Substitute each trait generic param with its concrete arg in the impl body
/// (the directive-copied fn signature plus the user's code block).
///
/// `trait_param_names` comes from the entry trait definition (`From<T>` →
/// `[T]`), paired positionally with `ImplParts::trait_generic_names` (the
/// spec-level args, `From<bool>` → `[bool]`). Token-level recursive: syn's
/// quote groups parameter tokens, so the replacement descends into groups.
/// Limitation: a *function* generic param that happens to share a trait
/// param's name would be substituted too (rare; renamed params avoid it).
pub(crate) fn substitute_trait_generics(
    parts: &mut ImplParts, trait_param_names: &[Ident],
) {
    let Some(body) = parts.body.take() else {
        return;
    };
    if trait_param_names.is_empty() || parts.trait_generic_names.is_empty() {
        parts.body = Some(body);
        return;
    }
    // Pair type/const param names with their concrete args, skipping lifetime
    // args (`'static` — a TokenStream starting with a `'` punct): bodies
    // reference their own impl lifetimes, never substituted trait args.
    let map = trait_param_names
        .iter()
        .zip(parts.trait_generic_names.iter().filter(|ts| {
            !matches!(
                (*ts).clone().into_iter().next(),
                Some(TokenTree::Punct(p)) if p.as_char() == '\''
            )
        }))
        .map(|(name, arg)| (name.clone(), arg.clone()))
        .collect::<Vec<_>>();
    parts.body = Some(replace_idents(body, &map));
}

/// Recursively replace every ident equal to a mapped trait param name.
fn replace_idents(ts: TokenStream, map: &[(Ident, TokenStream)]) -> TokenStream {
    ts.into_iter()
        .flat_map(|tt| match &tt {
            TokenTree::Ident(id) => map
                .iter()
                .find(|(name, _)| name == id)
                .map(|(_, repl)| repl.clone())
                .unwrap_or_else(|| TokenStream::from(tt.clone())),
            TokenTree::Group(g) => {
                let inner = replace_idents(g.stream(), map);
                let mut ng = proc_macro2::Group::new(g.delimiter(), inner);
                ng.set_span(g.span());
                TokenStream::from(TokenTree::Group(ng))
            }
            other => TokenStream::from(other.clone()),
        })
        .collect()
}

/// Expand splat elements inside `TyTuple` at the Ty-structure level (the
/// codegen postprocess — parse/apply/expand keep `*()`/`*[]` whole). A splat
/// element becomes its flat elements with fresh declarations hoisted:
/// `(A, *(B,C))` → `(A,B,C)`, `(*(()^3))` → `<P0,P1,P2>(P0,P1,P2)`.
/// Generic args (`T<*(A,B)>`) and trait args (`Conv<*(A,B)>`) expand here
/// too (via [`expand_tp`]) — since `TyTypeParam` stores params as `Box<Ty>`,
/// splats stay structural and need no token-level pass.
pub(crate) fn expand_splat_elems(ty: Ty) -> Ty {
    let Ty { span, kind } = ty;
    match kind {
        TyKind::Tuple(t) => {
            let (flat, decl) =
                t.0.into_iter().fold((vec![], None), |(mut flat, decl), e| {
                    if matches!(e.kind, TyKind::Splat(_)) {
                        let (mut es, d) = splat_expand(e);
                        flat.append(&mut es);
                        (flat, merge_decls(decl, d))
                    } else {
                        flat.push(expand_splat_elems(e));
                        (flat, decl)
                    }
                });
            let tuple = TyTuple(flat).to_ty().with_span(span);
            match decl {
                Some(d) => TyWithType(d, tuple.into()).to_ty().with_span(span),
                None => tuple,
            }
        }
        TyKind::Group(g) => {
            TyGroup(Box::new(expand_splat_elems(*g.0))).to_ty().with_span(span)
        }
        TyKind::WithCode(wc) => {
            let inner = wc.0.map(|e| expand_splat_elems(*e).into());
            TyWithCode(inner, wc.1).to_ty().with_span(span)
        }
        TyKind::WithType(wt) => TyWithType(wt.0, Box::new(expand_splat_elems(*wt.1)))
            .to_ty()
            .with_span(span),
        TyKind::WithTrait(wt) => {
            // The trait path itself may carry splat args (`Conv<*(A,B)>`) —
            // expand them via `expand_tp`, hoisting any `*()^N` declaration
            // into a `TyWithType` around the whole `WithTrait`.
            let (tp, decl) = expand_tp(wt.0.1);
            let trait_ty = TyTrait(wt.0.0, tp);
            let inner = Box::new(expand_splat_elems(*wt.1));
            match decl {
                Some(d) => {
                    TyWithType(d, Box::new(TyWithTrait(trait_ty, inner).to_ty()))
                        .to_ty()
                        .with_span(span)
                }
                None => TyWithTrait(trait_ty, inner).to_ty().with_span(span),
            }
        }
        TyKind::WithWhere(ww) => {
            let inner = ww.0.map(|e| expand_splat_elems(*e).into());
            TyWithWhere(inner, ww.1).to_ty().with_span(span)
        }
        TyKind::WithPrefix(wp) => {
            let inner = wp.1.map(|e| expand_splat_elems(*e).into());
            TyWithPrefix(wp.0, inner).to_ty().with_span(span)
        }
        TyKind::WithAttr(wa) => {
            let inner = wa.1.map(|e| expand_splat_elems(*e).into());
            TyWithAttr(wa.0, inner).to_ty().with_span(span)
        }
        TyKind::Generic(g) => {
            let (tp, decl) = expand_tp(g.1);
            let generic = TyGeneric(Box::new(expand_splat_elems(*g.0)), tp)
                .to_ty()
                .with_span(span);
            match decl {
                Some(d) => TyWithType(d, Box::new(generic)).to_ty().with_span(span),
                None => generic,
            }
        }
        TyKind::Trait(t) => {
            let (tp, decl) = expand_tp(t.1);
            let trait_ty = TyTrait(t.0, tp).to_ty().with_span(span);
            match decl {
                Some(d) => TyWithType(d, Box::new(trait_ty)).to_ty().with_span(span),
                None => trait_ty,
            }
        }
        // Leaves and token-stream-bearing nodes (Splat / PrimitiveArray /
        // Fn / ...) stay — a bare `Splat` is itself the pending expansion.
        other => Ty { span, kind: other },
    }
}

/// Expand splat params inside a `TyTypeParam` (generic args / trait args):
/// top-level splat params flatten via [`flat_splat_params`], then every
/// remaining param (name / bound / binding value) recurses through
/// [`expand_splat_elems`]. Fresh declarations hoisted out of `*()^N` splats
/// are returned for the caller to wrap in `TyWithType` (a `TyGeneric` /
/// `TyTrait` cannot carry them itself).
fn expand_tp(tp: TyTypeParam) -> (TyTypeParam, Option<TyTypeParam>) {
    let (flat, decl) = flat_splat_params(tp.params);
    let params = flat
        .into_iter()
        .map(|(name, bound)| {
            let name = expand_splat_elems(*name);
            let bound = bound.map(expand_splat_elems);
            (Box::new(name), bound)
        })
        .collect();
    let bindings = tp
        .bindings
        .into_iter()
        .map(|(n, v)| {
            (
                Box::new(expand_splat_elems(*n)),
                Box::new(expand_splat_elems(*v)),
            )
        })
        .collect();
    (TyTypeParam { params, bindings }, decl)
}
