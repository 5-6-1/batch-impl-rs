//! Trait item lookups (`#name` / `#fill` / `#delegate` resolve item
//! signatures from the annotated trait) plus the `@all`-family marker specs.

use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::ItemTrait;

use crate::preprocess::directives::name_list::{AllMarkerSpec, ReceiverFilter};
use crate::util::{compile_err, compile_error_str};

/// Resolves an `all`-family marker. `default=None` includes everything;
/// `Some(true)` only default impls; `Some(false)` only no-default (required);
/// `receiver` filters fn items by receiver kind (`None` = all). The directive
/// domain (`@all`) and the macro-meta layer (`@all`) share the same table.
pub(crate) fn resolve_all_marker(name: &str) -> Option<AllMarkerSpec> {
    match name {
        "all" => ((true, true, true), None, None).into(),
        "all_methods" => ((true, false, false), None, None).into(),
        "all_constants" => ((false, true, false), None, None).into(),
        "all_types" => ((false, false, true), None, None).into(),
        "all_default" => ((true, true, true), true.into(), None).into(),
        "all_default_methods" => ((true, false, false), true.into(), None).into(),
        "all_default_constants" => ((false, true, false), true.into(), None).into(),
        "all_default_types" => ((false, false, true), true.into(), None).into(),
        "all_required" => ((true, true, true), false.into(), None).into(),
        "all_required_methods" => ((true, false, false), false.into(), None).into(),
        "all_required_constants" => ((false, true, false), false.into(), None).into(),
        "all_required_types" => ((false, false, true), false.into(), None).into(),
        "all_ref_methods" => {
            ((true, false, false), None, ReceiverFilter::Ref.into()).into()
        }
        "all_value_methods" => {
            ((true, false, false), None, ReceiverFilter::Value.into()).into()
        }
        "all_static_methods" => {
            ((true, false, false), None, ReceiverFilter::Static.into()).into()
        }
        _ => None,
    }
}

/// Generic-parameter family markers (`@all_type_params` / `@all_const_params` /
/// `@all_lifetimes`): expand to a flat `<...>` generic declaration copied from
/// the trait's own generic parameters (type params by name — bounds are picked
/// up by codegen's same-name inheritance; const params need the full
/// `const N: usize` declaration (a bare name is E0747); lifetimes as-is).
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum GenericFilter {
    Type,
    Const,
    Lifetime,
}

pub(crate) fn resolve_generic_marker(name: &str) -> Option<GenericFilter> {
    match name {
        "all_type_params" => GenericFilter::Type.into(),
        "all_const_params" => GenericFilter::Const.into(),
        "all_lifetimes" => GenericFilter::Lifetime.into(),
        _ => None,
    }
}

/// Builds the `<...>` declaration for the selected parameter kind; `None`
/// when the trait has no parameters of that kind.
pub(crate) fn get_trait_generic_decl(
    trait_def: &ItemTrait, f: GenericFilter,
) -> Option<TokenStream> {
    let names = trait_def
        .generics
        .params
        .iter()
        .filter_map(|p| match (p, f) {
            (syn::GenericParam::Type(tp), GenericFilter::Type) => {
                let id = tp.ident.clone();
                quote!(#id).into()
            }
            (syn::GenericParam::Const(cp), GenericFilter::Const) => {
                quote!(#cp).into()
            }
            (syn::GenericParam::Lifetime(ld), GenericFilter::Lifetime) => {
                quote!(#ld).into()
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if names.is_empty() {
        return None;
    }
    quote::quote!(< #(#names),* >).into()
}

/// Collects trait item names. `include_*` controls the kinds; `default`
/// filters default-implementation state: `Some(true)` only those with a
/// default, `Some(false)` only those without (required), `None` all (syn's
/// `default` field: fn=default body, const=default value, type=default type);
/// `receiver` filters fn items by receiver kind (`None` = all).
pub(crate) fn get_trait_item_names(
    trait_def: &ItemTrait, include_fn: bool, include_const: bool, include_type: bool,
    default: Option<bool>, receiver: Option<ReceiverFilter>,
) -> Vec<Ident> {
    let mut names = vec![];
    for item in &trait_def.items {
        let (kind, has_default) = match item {
            syn::TraitItem::Fn(f) => (0u8, f.default.is_some()),
            syn::TraitItem::Const(c) => (1, c.default.is_some()),
            syn::TraitItem::Type(t) => (2, t.default.is_some()),
            _ => (3, false),
        };
        let include = match kind {
            0 => include_fn,
            1 => include_const,
            2 => include_type,
            _ => false,
        };
        let receiver_ok = match (kind, receiver) {
            (0, Some(rk)) => {
                // syn 3: `receiver()` returns `Option<&Receiver>` with a
                // `ReceiverKind` (Value / Reference / Typed).
                let rk_syn = match item {
                    syn::TraitItem::Fn(f) => f.sig.receiver().map(|r| &r.kind),
                    _ => None,
                };
                match rk {
                    ReceiverFilter::Ref => {
                        matches!(rk_syn, Some(syn::ReceiverKind::Reference(..)))
                    }
                    ReceiverFilter::Value => matches!(
                        rk_syn,
                        Some(syn::ReceiverKind::Value | syn::ReceiverKind::Typed(..))
                    ),
                    ReceiverFilter::Static => rk_syn.is_none(),
                }
            }
            _ => true,
        };
        if include && receiver_ok && default.is_none_or(|d| d == has_default) {
            match item {
                syn::TraitItem::Fn(f) => names.push(f.sig.ident.clone()),
                syn::TraitItem::Const(c) => names.push(c.ident.clone()),
                syn::TraitItem::Type(t) => names.push(t.ident.clone()),
                _ => {}
            }
        }
    }
    names
}

pub(crate) fn get_trait_item<'a>(
    trait_def: &'a ItemTrait, name: &Ident,
) -> Result<&'a syn::TraitItem, TokenStream> {
    for item in &trait_def.items {
        let found = match item {
            syn::TraitItem::Fn(f) => f.sig.ident == *name,
            syn::TraitItem::Const(c) => c.ident == *name,
            syn::TraitItem::Type(t) => t.ident == *name,
            _ => false,
        };
        if found {
            return Ok(item);
        }
    }
    Err(compile_err!(
        "batch-impl: item `{}` not found in trait `{}`",
        trait_def.ident,
        name
    ))
}

pub(crate) fn build_from_item(
    item: &syn::TraitItem, body: &TokenStream,
) -> TokenStream {
    build_from_item_sig(item, None, body)
}

/// Like [`build_from_item`], but with an optional signature override (used by
/// `#delegate`, which may have rewritten `_` wildcard params into named ones).
///
/// Trait generic substitution (`From<bool>`: `value: T` → `value: bool`) is
/// NOT done here — it is a codegen postprocess over `ImplParts`, which has
/// both the trait arg names and the full body.
pub(crate) fn build_from_item_sig(
    item: &syn::TraitItem, sig: Option<&syn::Signature>, body: &TokenStream,
) -> TokenStream {
    match item {
        syn::TraitItem::Fn(f) => {
            let mut f = f.clone();
            if let Some(s) = sig {
                f.sig = s.clone();
            }
            f.semi_token = None;
            f.default = syn::Block {
                brace_token: syn::token::Brace::default(),
                stmts: vec![syn::Stmt::Expr(syn::Expr::Verbatim(body.clone()), None)],
            }
            .into();
            quote! {#f}
        }
        syn::TraitItem::Const(c) => {
            let mut c = c.clone();
            c.default =
                (syn::token::Eq::default(), syn::Expr::Verbatim(body.clone())).into();
            quote! {#c}
        }
        syn::TraitItem::Type(t) => {
            let mut t = t.clone();
            t.default =
                (syn::token::Eq::default(), syn::Type::Verbatim(body.clone())).into();
            quote! {#t}
        }
        _ => compile_error_str(
            "invalid item form; this error cannot occur",
            proc_macro2::Span::call_site(),
        ),
    }
}
