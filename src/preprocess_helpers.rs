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
    parse_name_tokens(tokens, trait_def, "指令参数")
}

/// 解析指令参数为 item 名列表：`#all` 系列标记、逗号分隔的标识符列表、
/// 以及 `-name` 排除项（保留列表减去排除列表，如 `#fill(#all,-foo)`）。
///
/// 指令参数域里 `-` 此前无语义（参数只解析标识符/逗号），专用于列表减法，
/// 不与类型 DSL 的 `-` 连接运算符冲突（DSL 解析不进入指令参数）。
/// `what` 用于诊断措辞（主参数为"指令参数"）。
fn parse_name_tokens(
    tokens: &[TokenTree], trait_def: &ItemTrait, what: &str,
) -> Result<Vec<Ident>, TokenStream> {
    if tokens.is_empty() {
        return Err(compile_error_str(&format!("batch-impl: {}不能为空", what)));
    }
    let mut keep: Vec<Ident> = vec![];
    let mut exclude: Vec<Ident> = vec![];
    let mut prev_was_comma = true; // 起始视为"刚经过逗号"，用于拦截前导逗号
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            TokenTree::Ident(id) => {
                keep.push(Ident::new(&id.to_string(), id.span()));
                prev_was_comma = false;
                i += 1;
            }
            TokenTree::Punct(p) if p.as_char() == ',' => {
                if prev_was_comma {
                    return Err(compile_error_str(&format!(
                        "batch-impl: {}中逗号位置不合法（不允许前导/尾随/连续逗号）",
                        what
                    )));
                }
                prev_was_comma = true;
                i += 1;
            }
            // `-name` / `-#all`：排除项（排除优先于保留）
            TokenTree::Punct(p) if p.as_char() == '-' => {
                let (ids, consumed) =
                    parse_minus_target(&tokens[i + 1..], trait_def, what)?;
                exclude.extend(ids);
                i += 1 + consumed;
                prev_was_comma = false;
            }
            // `#all` 系列标记：展开为对应 item 列表（并入保留列表）
            TokenTree::Punct(p) if p.as_char() == '#' => {
                let (ids, consumed) =
                    parse_marker(&tokens[i + 1..], trait_def, what)?;
                keep.extend(ids);
                i += 1 + consumed;
                prev_was_comma = false;
            }
            _ => {
                return Err(compile_error_str(&format!(
                    "batch-impl: {}中期望标识符、逗号或 `-` 排除项，得到 `{}`",
                    what, tokens[i]
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
    let names: Vec<Ident> =
        keep.into_iter().filter(|id| !exclude.iter().any(|e| e == id)).collect();
    if names.is_empty() {
        return Err(compile_error_str(&format!("batch-impl: {}不能为空", what)));
    }
    Ok(names)
}

/// `-` 后的目标：标识符（`-foo`）或 `#all` 系列标记（`-#all_methods`）。
/// 返回（展开的 item 名列表, 消费的 token 数）。
fn parse_minus_target(
    tokens: &[TokenTree], trait_def: &ItemTrait, what: &str,
) -> Result<(Vec<Ident>, usize), TokenStream> {
    match tokens.first() {
        Some(TokenTree::Ident(id)) => {
            Ok((vec![Ident::new(&id.to_string(), id.span())], 1))
        }
        Some(TokenTree::Punct(p)) if p.as_char() == '#' => {
            let (ids, n) = parse_marker(&tokens[1..], trait_def, what)?;
            Ok((ids, 1 + n))
        }
        _ => Err(compile_error_str(&format!(
            "batch-impl: {}中 `-` 后期望标识符或 `#all` 标记（如 `-foo`、`-#all_methods`）",
            what
        ))),
    }
}

/// `#all` 系列标记展开（`#all` / `#all_methods` / `#all_constants` / `#all_types`）。
/// 返回（展开的 item 名列表, 消费的 token 数）。
fn parse_marker(
    tokens: &[TokenTree], trait_def: &ItemTrait, what: &str,
) -> Result<(Vec<Ident>, usize), TokenStream> {
    let Some(TokenTree::Ident(id)) = tokens.first() else {
        return Err(compile_error_str(&format!(
            "batch-impl: {}中 `#` 后期望 `#all`/`#all_methods`/`#all_constants`/`#all_types` 标记",
            what
        )));
    };
    let ids = if id == "all_methods" {
        get_all_trait_methods(trait_def)
    } else if id == "all" {
        get_all_trait_items(trait_def)
    } else if id == "all_constants" {
        get_all_trait_constants(trait_def)
    } else if id == "all_types" {
        get_all_trait_types(trait_def)
    } else {
        return Err(compile_error_str(&format!(
            "batch-impl: {}中未知的 `#{}` 标记（支持 `#all`/`#all_methods`/`#all_constants`/`#all_types`）",
            what, id
        )));
    };
    Ok((ids, 1))
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
