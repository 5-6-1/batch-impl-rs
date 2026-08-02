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
    // `#except(保留){排除}`：保留列表减去排除列表。两个列表各自是
    // `#all` 系列标记或逗号分隔的标识符列表，最终得到一个 item 名列表。
    // 如 `#fill(#except(#all){foo}){...}` = 所有 item 除 `foo`。
    if let [TokenTree::Punct(h), TokenTree::Ident(n), rest @ ..] = tokens
        && h.as_char() == '#'
        && n == "except"
    {
        let [TokenTree::Group(keep), TokenTree::Group(excl)] = rest else {
            return Err(compile_error_str(
                "batch-impl: `#except` 期望 `(保留列表){排除列表}` 两个括号参数",
            ));
        };
        let keep_ts = keep.stream().into_iter().collect::<Vec<_>>();
        let excl_ts = excl.stream().into_iter().collect::<Vec<_>>();
        let keep_ids =
            parse_name_tokens(&keep_ts, trait_def, "`#except` 的保留列表")?;
        let excl_ids =
            parse_name_tokens(&excl_ts, trait_def, "`#except` 的排除列表")?;
        return Ok(keep_ids
            .into_iter()
            .filter(|id| !excl_ids.iter().any(|e| e == id))
            .collect());
    }
    parse_name_tokens(tokens, trait_def, "指令参数")
}

/// 解析指令参数为 item 名列表：`#all` 系列标记，或逗号分隔的标识符列表。
/// `what` 用于诊断措辞（主参数为"指令参数"，`#except` 的子列表各自带上下文）。
fn parse_name_tokens(
    tokens: &[TokenTree], trait_def: &ItemTrait, what: &str,
) -> Result<Vec<Ident>, TokenStream> {
    if tokens.is_empty() {
        return Err(compile_error_str(&format!("batch-impl: {}不能为空", what)));
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
    // 逗号分隔的标识符列表：前导/尾随/连续逗号视为笔误报错，其余 token 必须是标识符。
    let mut names = vec![];
    let mut prev_was_comma = true; // 起始视为"刚经过逗号"，用于拦截前导逗号
    for t in tokens {
        match t {
            TokenTree::Ident(id) => {
                names.push(Ident::new(&id.to_string(), id.span()));
                prev_was_comma = false;
            }
            TokenTree::Punct(p) if p.as_char() == ',' => {
                if prev_was_comma {
                    return Err(compile_error_str(&format!(
                        "batch-impl: {}中逗号位置不合法（不允许前导/尾随/连续逗号）",
                        what
                    )));
                }
                prev_was_comma = true;
            }
            _ => {
                return Err(compile_error_str(&format!(
                    "batch-impl: {}中期望标识符或逗号，得到 `{}`",
                    what, t
                )));
            }
        }
    }
    if prev_was_comma {
        return Err(compile_error_str(&format!(
            "batch-impl: {}中逗号位置不合法（不允许前导/尾随/连续逗号）",
            what
        )));
    }
    if names.is_empty() {
        return Err(compile_error_str(&format!("batch-impl: {}不能为空", what)));
    }
    Ok(names)
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

/// 收集委托调用要转发的参数标识符（跳过 `self` 接收者）。
///
/// 仅支持 `self` 与纯标识符模式；解构模式（如 `(a, b)`、`_`）无法按名转发，
/// 返回包含该模式文本的 `Err`，由调用方构造诊断。
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
