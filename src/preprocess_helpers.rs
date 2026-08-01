use proc_macro2::{Ident, TokenStream, TokenTree};
use quote::quote;
use syn::ItemTrait;

use crate::diagnostic::compile_error_str;

pub(crate) fn parse_names_from_tokens(
    tokens: &[TokenTree], trait_def: &ItemTrait,
) -> Result<Vec<Ident>, TokenStream> {
    if tokens.is_empty() {
        return Err(compile_error_str("batch-impl: 指令的参数列表不能为空"));
    }
    if tokens.len() == 2
        && let (TokenTree::Punct(p), TokenTree::Ident(id)) = (&tokens[0], &tokens[1])
        && p.as_char() == '#'
    {
        if id == "all_methods" {
            return Ok(get_all_trait_methods(trait_def));
        } else if id == "all" {
            return Ok(get_all_trait_items(trait_def));
        } else if id == "all_constants" {
            return Ok(get_all_trait_constants(trait_def));
        } else if id == "all_types" {
            return Ok(get_all_trait_types(trait_def));
        }
    }
    tokens
        .iter()
        .map(|t| {
            if let TokenTree::Ident(id) = t {
                Ok(Ident::new(&id.to_string(), id.span()))
            } else if let TokenTree::Punct(p) = t
                && p.as_char() == ','
            {
                Err(None)
            } else {
                Err(compile_error_str(&format!(
                    "batch-impl: 指令参数中期望标识符或逗号，得到 `{}`",
                    t
                ))
                .into())
            }
        })
        .filter_map(|r| match r {
            Ok(v) => Ok(v).into(),
            Err(None) => None,
            Err(Some(e)) => Err(e).into(),
        })
        .collect()
}

fn get_trait_item_names(
    trait_def: &ItemTrait, include_fn: bool, include_const: bool, include_type: bool,
) -> Vec<Ident> {
    let mut names = vec![];
    for item in &trait_def.items {
        if include_fn && let syn::TraitItem::Fn(f) = item {
            names.push(f.sig.ident.clone());
        } else if include_const && let syn::TraitItem::Const(c) = item {
            names.push(c.ident.clone());
        } else if include_type && let syn::TraitItem::Type(t) = item {
            names.push(t.ident.clone());
        }
    }
    names
}

fn get_all_trait_methods(trait_def: &ItemTrait) -> Vec<Ident> {
    get_trait_item_names(trait_def, true, false, false)
}

fn get_all_trait_items(trait_def: &ItemTrait) -> Vec<Ident> {
    get_trait_item_names(trait_def, true, true, true)
}

fn get_all_trait_constants(trait_def: &ItemTrait) -> Vec<Ident> {
    get_trait_item_names(trait_def, false, true, false)
}

fn get_all_trait_types(trait_def: &ItemTrait) -> Vec<Ident> {
    get_trait_item_names(trait_def, false, false, true)
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
    Err(compile_error_str(&format!(
        "batch-impl: trait `{}` 中没有找到 item `{}`",
        trait_def.ident, name
    )))
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
        _ => compile_error_str("item格式错误，不可能出现的错误"),
    }
}

pub(crate) fn collect_call_args(sig: &syn::Signature) -> Vec<Ident> {
    sig.inputs
        .iter()
        .filter_map(|arg| {
            if let syn::FnArg::Typed(pat_type) = arg
                && let syn::Pat::Ident(pat_ident) = &*pat_type.pat
            {
                return pat_ident.ident.clone().into();
            }
            None
        })
        .collect()
}
