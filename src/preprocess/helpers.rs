use proc_macro2::{Ident, TokenStream, TokenTree};
use quote::quote;
use syn::ItemTrait;

use crate::util::{compile_err, compile_error_str};

pub(crate) fn parse_names_from_tokens(
    tokens: &[TokenTree], trait_def: &ItemTrait,
) -> Result<Vec<Ident>, TokenStream> {
    if tokens.is_empty() {
        return Err(compile_error_str(
            "batch-impl: the directive's argument list cannot be empty",
            proc_macro2::Span::call_site(),
        ));
    }
    parse_name_tokens(tokens, trait_def, "directive arguments")
}

/// Parses directive arguments into an item-name list: `@all`-family markers,
/// comma-separated identifier lists, and `-name` exclusions (keep list minus
/// exclude list, e.g. `#fill(@all,-foo)`).
///
/// In the directive-argument domain `-` had no meaning before (arguments
/// parse only identifiers/commas) and is dedicated to list subtraction; it
/// does not clash with the `-` join operator of the type DSL (DSL parsing
/// never enters directive arguments). `what` is used for diagnostic wording
/// (the main args are "directive arguments").
fn parse_name_tokens(
    tokens: &[TokenTree], trait_def: &ItemTrait, what: &str,
) -> Result<Vec<Ident>, TokenStream> {
    if tokens.is_empty() {
        return Err(compile_err!("batch-impl: {} cannot be empty", what));
    }
    let mut keep: Vec<Ident> = vec![];
    let mut exclude: Vec<Ident> = vec![];
    let mut prev_was_comma = true; // Start is treated as "just passed a comma", to catch a leading comma
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            TokenTree::Ident(id) => {
                keep.push(Ident::new(&id.to_string(), id.span()));
                prev_was_comma = false;
                i += 1;
            }
            // `[a, b]` list: parse the group contents into names recursively
            // (`@all` family expansions have this shape; users may also
            // hand-write `[a,b]` or `-[a,b]` exclusions; an empty group
            // errors "cannot be empty" via recursion)
            TokenTree::Group(g) if g.delimiter() == delimiter![[]] => {
                let inner: Vec<_> = g.stream().into_iter().collect();
                keep.extend(parse_name_tokens(&inner, trait_def, what)?);
                prev_was_comma = false;
                i += 1;
            }
            TokenTree::Punct(p) if p.as_char() == ',' => {
                if prev_was_comma {
                    return Err(compile_err!(
                        "batch-impl: in {}, a comma is in an illegal position \
                         (no leading/trailing/consecutive commas)",
                        what
                    ));
                }
                prev_was_comma = true;
                i += 1;
            }
            // `-name` / `-[a,b]` / `-@all` (@all expands to a Bracket group
            // and takes the group branch): exclusion
            TokenTree::Punct(p) if p.as_char() == '-' => {
                let (ids, consumed) =
                    parse_minus_target(&tokens[i + 1..], trait_def, what)?;
                exclude.extend(ids);
                i += 1 + consumed;
                prev_was_comma = false;
            }
            // `#` no longer appears in the directive-argument domain: `#`
            // remains only as the directive-name format; scope selection
            // belongs to the `@all` family
            _ => {
                return Err(compile_err!(
                    "batch-impl: in {}, expected an identifier, comma, `[...]` \
                     list, or `-` exclusion, got `{}`",
                    what,
                    tokens[i]
                ));
            }
        }
    }
    if prev_was_comma {
        return Err(compile_err!(
            "batch-impl: in {}, a comma is in an illegal position \
             (no leading/trailing/consecutive commas)",
            what
        ));
    }
    let names: Vec<Ident> =
        keep.into_iter().filter(|id| !exclude.iter().any(|e| e == id)).collect();
    if names.is_empty() {
        return Err(compile_err!("batch-impl: {} cannot be empty", what));
    }
    Ok(names)
}

/// The target after `-`: an identifier (`-foo`) or an `@all`-family marker
/// (`-@all_methods`). Returns (expanded item-name list, tokens consumed).
fn parse_minus_target(
    tokens: &[TokenTree], trait_def: &ItemTrait, what: &str,
) -> Result<(Vec<Ident>, usize), TokenStream> {
    match tokens.first() {
        Some(TokenTree::Ident(id)) => {
            Ok((vec![Ident::new(&id.to_string(), id.span())], 1))
        }
        Some(TokenTree::Group(g)) if g.delimiter() == delimiter![[]] => {
            let inner: Vec<_> = g.stream().into_iter().collect();
            let ids = parse_name_tokens(&inner, trait_def, what)?;
            Ok((ids, 1))
        }
        _ => Err(compile_err!(
            "batch-impl: in {}, after `-` expected an identifier or `[...]` \
             list (e.g. `-foo`, `-[a,b]`)",
            what
        )),
    }
}

/// Receiver kind filter for fn items (`@all_ref_methods` etc.).
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ReceiverFilter {
    /// `&self` / `&mut self`
    Ref,
    /// `self` (by-value, including typed receivers like `self: Box<Self>`)
    Value,
    /// no receiver — an associated function (e.g. `fn new() -> Self`)
    Static,
}

/// `all`-family marker → (include_fn, include_const, include_type, default
/// filter, receiver filter).
pub(crate) type AllMarkerSpec =
    ((bool, bool, bool), Option<bool>, Option<ReceiverFilter>);

/// Resolves an `all`-family marker. `default=None` includes everything;
/// `Some(true)` only default impls; `Some(false)` only no-default (required);
/// `receiver` filters fn items by receiver kind (`None` = all). The directive
/// domain (`@all`) and the macro-meta layer (`@all`) share the same table.
pub(crate) fn resolve_all_marker(name: &str) -> Option<AllMarkerSpec> {
    match name {
        "all" => Some(((true, true, true), None, None)),
        "all_methods" => Some(((true, false, false), None, None)),
        "all_constants" => Some(((false, true, false), None, None)),
        "all_types" => Some(((false, false, true), None, None)),
        "all_default" => Some(((true, true, true), Some(true), None)),
        "all_default_methods" => Some(((true, false, false), Some(true), None)),
        "all_default_constants" => Some(((false, true, false), Some(true), None)),
        "all_default_types" => Some(((false, false, true), Some(true), None)),
        "all_required" => Some(((true, true, true), Some(false), None)),
        "all_required_methods" => Some(((true, false, false), Some(false), None)),
        "all_required_constants" => Some(((false, true, false), Some(false), None)),
        "all_required_types" => Some(((false, false, true), Some(false), None)),
        "all_ref_methods" => {
            Some(((true, false, false), None, Some(ReceiverFilter::Ref)))
        }
        "all_value_methods" => {
            Some(((true, false, false), None, Some(ReceiverFilter::Value)))
        }
        "all_static_methods" => {
            Some(((true, false, false), None, Some(ReceiverFilter::Static)))
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
        "all_type_params" => Some(GenericFilter::Type),
        "all_const_params" => Some(GenericFilter::Const),
        "all_lifetimes" => Some(GenericFilter::Lifetime),
        _ => None,
    }
}

/// Builds the `<...>` declaration for the selected parameter kind; `None`
/// when the trait has no parameters of that kind.
pub(crate) fn get_trait_generic_decl(
    trait_def: &ItemTrait, f: GenericFilter,
) -> Option<proc_macro2::TokenStream> {
    use proc_macro2::TokenStream;
    use quote::quote;
    let names: Vec<TokenStream> = trait_def
        .generics
        .params
        .iter()
        .filter_map(|p| match (p, f) {
            (syn::GenericParam::Type(tp), GenericFilter::Type) => {
                let id = &tp.ident;
                Some(quote!(#id))
            }
            (syn::GenericParam::Const(cp), GenericFilter::Const) => Some(quote!(#cp)),
            (syn::GenericParam::Lifetime(ld), GenericFilter::Lifetime) => Some(quote!(#ld)),
            _ => None,
        })
        .collect();
    if names.is_empty() {
        return None;
    }
    Some(quote!(< #(#names),* >))
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
    match item {
        syn::TraitItem::Fn(f) => {
            let mut f = f.clone();
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

/// Collects the argument identifiers to forward in a delegation call
/// (skipping the `self` receiver).
///
/// Only `self` and plain identifier patterns are supported; non-identifier
/// patterns (e.g. `(a, b)`, `_`) cannot be forwarded by name, and an `Err`
/// containing that pattern's text is returned for the caller to build a
/// diagnostic from.
pub(crate) fn collect_call_args(sig: &syn::Signature) -> Result<Vec<Ident>, String> {
    let mut args = vec![];
    for arg in &sig.inputs {
        match arg {
            syn::FnArg::Receiver(_) => {}
            syn::FnArg::Typed(pat_type) => {
                if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                    args.push(pat_ident.ident.clone());
                } else {
                    return Err(quote!(#pat_type).to_string());
                }
            }
        }
    }
    Ok(args)
}
