//! Helper functions of the `#blanket` directive (kept under the 350-line
//! cap by living in their own file): `Self`-return detection, the `@0`
//! target marker, grouped trait-path rendering, and wrapper-where
//! `@trait` resolution.

use proc_macro2::{Group, TokenStream, TokenTree};
use quote::{ToTokens, quote};

use crate::util::compile_error_str;

/// Forwards method type/const arguments explicitly. Lifetimes remain inferred:
/// supplying late-bound lifetimes in a call's turbofish is rejected by Rust.
pub(crate) fn method_turbofish(generics: &syn::Generics) -> TokenStream {
    let mut generics = generics.clone();
    generics.params = generics
        .params
        .into_iter()
        .filter(|p| !crate::ast::ParamKind::of_generic_param(p).is_lifetime())
        .collect();
    let (_, args, _) = generics.split_for_impl();
    args.as_turbofish().to_token_stream()
}

/// Spell the receiver's borrow explicitly for a qualified function call.
/// Explicit `self: &Self` / `self: &mut Self` use the same reference layer
/// as their shorthand forms; other typed receivers are passed by value.
pub(crate) fn receiver_borrow(receiver: &syn::Receiver) -> TokenStream {
    match &receiver.kind {
        syn::ReceiverKind::Reference(and, _, mutability) => quote!(#and #mutability),
        syn::ReceiverKind::Typed(_, ty) => match ty.as_ref() {
            syn::Type::Reference(reference) => {
                let and = &reference.and_token;
                let mutability = &reference.mutability;
                quote!(#and #mutability)
            }
            _ => TokenStream::new(),
        },
        _ => TokenStream::new(),
    }
}

/// Whether the generated forward for this receiver moves the inner value out of the wrapper.
/// `syn` reports `self`, `self: Self`, `self: &Self` and `self: &mut Self` all as `Value` or
/// `Typed`, so a `Typed` receiver has to be read one level deeper: only a **non-reference** type is
/// by-value. This is the **single** predicate behind two consumers - the `#[doc]` note that
/// `#blanket` emits and the `@all_ref_methods` / `@all_value_methods` selector. Probe D's F1
/// measured them apart: the selector matched `Typed(..)` blind, so a `self: &Self` method was
/// *dropped* by `@all_ref_methods` (E0046) and *selected* by `@all_value_methods` while the body
/// forwarded by reference and the note stayed silent.
pub(crate) fn forward_moves_the_value(kind: &syn::ReceiverKind) -> bool {
    match kind {
        syn::ReceiverKind::Value => true,
        syn::ReceiverKind::Typed(_, ty) => !matches!(**ty, syn::Type::Reference(_)),
        syn::ReceiverKind::Reference(..) => false,
        // `ReceiverKind` is `#[non_exhaustive]`, so a receiver form a future `syn` adds lands here.
        // Not by-value is the safe reading: the note only ever *advises* a hand-written body, and
        // claiming it for a shared receiver is the mistake probe A's F8 measured.
        _ => false,
    }
}

/// Whether a method's return type references **bare `Self`** (making blanket/// delegation unsound: the forwarded call returns the inner type, not the
/// wrapper's `Self`). `Self::Assoc` (an associated-type projection) is
/// **allowed**: it resolves through the projected item
/// (`type Output = <T as Trait>::Output;` — the wrapper's `Self::Output` is
/// `T::Output`), so it type-checks when the selection covers the associated
/// item and fails naturally (E0046) when it does not.
pub(crate) fn return_type_refs_self(output: &syn::ReturnType) -> bool {
    match output {
        syn::ReturnType::Default => false,
        syn::ReturnType::Type(_, ty) => ty_refs_bare_self(ty),
    }
}

/// Whether a method refers to bare `Self` in a parameter, return type, or
/// generic constraint. A constraint such as `U: Marker<Self>` applies to
/// different types on the wrapper and its deref target, just like a `Self`
/// parameter or return. Associated projections stay allowed, as does the
/// ordinary sizedness/outlives gates; Rust checks the target's corresponding
/// obligations itself.
/// Receiver `Self` is intentionally excluded: its wrapper layers are
/// removed by delegation, rather than forwarded as an ordinary argument.
pub(crate) fn sig_refs_bare_self(sig: &syn::Signature) -> bool {
    return_type_refs_self(&sig.output)
        || sig
            .inputs
            .iter()
            .any(|i| matches!(i, syn::FnArg::Typed(pt) if ty_refs_bare_self(&pt.ty)))
        || sig.generics.params.iter().any(|param| {
            // Attribute payloads are independent token languages, not type
            // constraints (e.g. `#[cfg_attr(any(), marker(Self))]`).
            let tokens = match param {
                syn::GenericParam::Type(param) => {
                    let bounds = &param.bounds;
                    let default = param.default.as_ref().map(|(_, ty)| ty);
                    quote!(#bounds #default)
                }
                syn::GenericParam::Const(param) => {
                    let ty = &param.ty;
                    let default = param.default.as_ref().map(|(_, expr)| expr);
                    quote!(#ty #default)
                }
                _ => return false,
            };
            ty_tokens_refs_bare_self(&tokens.into_iter().collect::<Vec<_>>())
        })
        || sig.generics.where_clause.as_ref().is_some_and(|clause| {
            clause.predicates.iter().any(|pred| {
                let syn::WherePredicate::Type(typed) = pred else { return false };
                let ty = &typed.bounded_ty;
                let bounds = &typed.bounds;
                !is_self_gate(pred)
                    && ty_tokens_refs_bare_self(
                        &quote!(#ty #bounds).into_iter().collect::<Vec<_>>(),
                    )
            })
        })
}

/// Preserve sizedness and outlives gates, including qualified standard-library
/// `Sized` spellings. They do not pass `Self` to another type; the delegated
/// borrow/move must still satisfy these obligations through ordinary Rust.
fn is_self_gate(pred: &syn::WherePredicate) -> bool {
    let syn::WherePredicate::Type(pred) = pred else { return false };
    if !matches!(&pred.bounded_ty, syn::Type::Path(p) if p.qself.is_none() && p.path.is_ident("Self"))
    {
        return false;
    }
    !pred.bounds.is_empty()
        && pred.bounds.iter().all(|bound| {
            let bound = match bound {
                syn::TypeParamBound::Lifetime(_) => return true,
                syn::TypeParamBound::Trait(bound) => bound,
                _ => return false,
            };
            if bound.maybe.is_some()
                || bound.modifiers.require_empty().is_err()
                || bound.lifetimes.is_some()
                || bound.path.segments.iter().any(|s| !s.arguments.is_empty())
            {
                return false;
            }
            let names = bound.path.segments.iter().map(|s| s.ident.to_string()).collect::<Vec<_>>();
            matches!(names.as_slice(), [name] if name == "Sized")
                || matches!(names.as_slice(), [root, module, name]
                if (root == "core" || root == "std") && module == "marker" && name == "Sized")
        })
}

/// A type's token stream references bare `Self` (not `Self::`) — recursing
/// into groups, so a `Self` inside `(Self, u8)` / `Box<Self>` is caught too
/// (a group is a single top-level token; a top-level-only scan would miss
/// it and emit a delegation that silently forwards the wrong type).
fn ty_refs_bare_self(ty: &syn::Type) -> bool {
    let tokens = ty.to_token_stream().into_iter().collect::<Vec<_>>();
    ty_tokens_refs_bare_self(&tokens)
}

fn ty_tokens_refs_bare_self(tokens: &[TokenTree]) -> bool {
    tokens.iter().enumerate().any(|(i, tt)| match tt {
        TokenTree::Ident(id) if id == "Self" => {
            // `Self::` — an associated projection, allowed (see above)
            !(matches!(tokens.get(i + 1), Some(TokenTree::Punct(p)) if p.as_char() == ':')
                && matches!(tokens.get(i + 2), Some(TokenTree::Punct(p)) if p.as_char() == ':'))
        }
        TokenTree::Group(g) => {
            ty_tokens_refs_bare_self(&g.stream().into_iter().collect::<Vec<_>>())
        }
        _ => false,
    })
}

/// Whether a wrapper's main part contains the `@0` target marker (`@` +
/// literal `0`, possibly nested inside groups) — the position decision only;
/// the marker itself is resolved by the parse layer into the fresh name.
pub(crate) fn has_at0(tokens: &[TokenTree]) -> bool {
    let v = tokens.to_vec();
    v.iter().enumerate().any(|(i, tt)| match tt {
        TokenTree::Punct(p) if p.as_char() == '@' => {
            matches!(v.get(i + 1), Some(TokenTree::Literal(l)) if l.to_string() == "0")
        }
        TokenTree::Group(g) => has_at0(&g.stream().into_iter().collect::<Vec<_>>()),
        _ => false,
    })
}

/// `Trait<X, Y>` with grouped angle args — blanket runs after `angle_collect`
/// and its output is no longer paired, so the group is built manually. An
/// empty param list yields the bare path.
pub(crate) fn trait_with_args(path: &TokenStream, param_names: &[TokenStream]) -> TokenStream {
    if param_names.is_empty() {
        quote!(#path)
    } else {
        let args_group = Group::new(delimiter![<>], quote!(#(#param_names),*));
        quote!(#path #args_group)
    }
}

/// Replaces `@trait` in wrapper where predicates with the full trait path
/// (local name, or the `#ext::Trait:` external path for `batch_impl_only`).
/// `@N` position references are **kept as-is** and resolved by codegen's
/// `resolve_where_at` like any user where predicate (blanket's fresh generic
/// is the only fresh, so `@0` indexes it); other tokens after `@` error.
pub(crate) fn resolve_target_predicates(
    preds: &[TokenTree], trait_full_path: &TokenStream,
) -> Result<Vec<TokenTree>, TokenStream> {
    let mut out = vec![];
    let mut i = 0;
    while let Some(cur) = preds.get(i) {
        match cur {
            TokenTree::Punct(p) if p.as_char() == '@' => match preds.get(i + 1) {
                Some(TokenTree::Ident(id)) if id == "trait" => {
                    out.extend(trait_full_path.clone());
                    i += 2;
                }
                // `@0` / `@N`: keep as-is for codegen; other forms error
                Some(TokenTree::Literal(lit)) if lit.to_string().parse::<usize>().is_ok() => {
                    out.push(cur.clone());
                    out.push(TokenTree::Literal(lit.clone()));
                    i += 2;
                }
                _ => {
                    return Err(compile_error_str(
                        "batch-impl: in #blanket wrapper where, `@` must be \
                         followed by a position digit (e.g. `@0`) or `@trait`",
                        cur.span(),
                    ));
                }
            },
            _ => {
                // Recurse into groups: `Vec<@trait>` in a wrapper where
                // predicate must substitute the trait path inside the group
                // too (a top-level-only scan would leak `@trait` into the
                // output — same recursion the sibling `has_at0` performs).
                if let TokenTree::Group(g) = cur {
                    let inner = g.stream().into_iter().collect::<Vec<_>>();
                    let resolved = resolve_target_predicates(&inner, trait_full_path)?;
                    let mut ng = Group::new(g.delimiter(), resolved.into_iter().collect());
                    ng.set_span(g.span());
                    out.push(TokenTree::Group(ng));
                    i += 1;
                } else {
                    out.push(cur.clone());
                    i += 1;
                }
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blanket_rejects_self_in_method_constraints() {
        for method in [
            quote!(
                fn inline<U: Marker<Self>>(&self);
            ),
            quote!(
                fn nested<U: Marker<(Self,)>>(&self);
            ),
            quote!(
                fn predicate<U>(&self)
                where
                    U: Marker<Self>;
            ),
            quote!(
                fn bounded_self<U>(&self)
                where
                    Self: Marker<U>;
            ),
            quote!(
                fn higher_ranked<U>(&self)
                where
                    U: for<'a> Marker<&'a Self>;
            ),
        ] {
            let item = syn::parse2(quote!(trait Rejected { #method })).unwrap();
            let error =
                crate::entry::expand_attr_macro("#blanket(@all){Box}".parse().unwrap(), item, true)
                    .unwrap_err()
                    .to_string();
            assert!(error.contains("references bare `Self`"), "{error}");
        }
    }

    #[test]
    fn blanket_allows_self_projections_receivers_and_sized_gates() {
        for method in [
            quote!(
                fn consume(self)
                where
                    Self: Sized;
            ),
            quote!(
                fn consume(self)
                where
                    Self: ::core::marker::Sized;
            ),
            quote!(
                fn borrowed<'a>(&'a self)
                where
                    Self: Sized + 'a;
            ),
            quote!(
                fn projected<U: Marker<Self::Item>>(&self) -> Self::Item
                where
                    U: Into<Self::Item>;
            ),
            quote!(
                fn typed(self: Box<Self>);
            ),
            quote!(
                fn decorated<#[cfg_attr(any(), marker(Self))] U>(&self);
            ),
        ] {
            let method: syn::TraitItemFn = syn::parse2(method).unwrap();
            assert!(!sig_refs_bare_self(&method.sig));
        }
    }
}
