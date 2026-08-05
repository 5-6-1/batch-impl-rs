use proc_macro2::{Ident, TokenStream, TokenTree};
use quote::quote;
use syn::ItemTrait;

use crate::util::{compile_err, compile_error_str};

pub(crate) fn parse_names_from_tokens(
    tokens: &[TokenTree], trait_def: &ItemTrait,
) -> Result<Vec<Ident>, TokenStream> {
    if tokens.is_empty() {
        return Err(compile_error_str("batch-impl: 指令的参数列表不能为空"));
    }
    parse_name_tokens(tokens, trait_def, "指令参数")
}

/// 解析指令参数为 item 名列表：`@all` 系列标记、逗号分隔的标识符列表、
/// 以及 `-name` 排除项（保留列表减去排除列表，如 `#fill(@all,-foo)`）。
///
/// 指令参数域里 `-` 此前无语义（参数只解析标识符/逗号），专用于列表减法，
/// 不与类型 DSL 的 `-` 连接运算符冲突（DSL 解析不进入指令参数）。
/// `what` 用于诊断措辞（主参数为"指令参数"）。
fn parse_name_tokens(
    tokens: &[TokenTree], trait_def: &ItemTrait, what: &str,
) -> Result<Vec<Ident>, TokenStream> {
    if tokens.is_empty() {
        return Err(compile_err!("batch-impl: {}不能为空", what));
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
            // `[a, b]` 列表：递归解析组内容为名字（`@all` 系展开产物即此形态；
            // 用户也可手写 `[a,b]` 或 `-[a,b]` 排除；空组经递归报"不能为空"）
            TokenTree::Group(g) if g.delimiter() == delimiter![[]] => {
                let inner: Vec<_> = g.stream().into_iter().collect();
                keep.extend(parse_name_tokens(&inner, trait_def, what)?);
                prev_was_comma = false;
                i += 1;
            }
            TokenTree::Punct(p) if p.as_char() == ',' => {
                if prev_was_comma {
                    return Err(compile_err!(
                        "batch-impl: {}中逗号位置不合法（不允许前导/尾随/连续逗号）",
                        what
                    ));
                }
                prev_was_comma = true;
                i += 1;
            }
            // `-name` / `-[a,b]` / `-@all`（@all 展开为 Bracket 组后走组分支）：排除项
            TokenTree::Punct(p) if p.as_char() == '-' => {
                let (ids, consumed) =
                    parse_minus_target(&tokens[i + 1..], trait_def, what)?;
                exclude.extend(ids);
                i += 1 + consumed;
                prev_was_comma = false;
            }
            // `#` 不再出现在指令参数域：`#` 只剩指令名一种格式，范围选择归 `@all` 系
            _ => {
                return Err(compile_err!(
                    "batch-impl: {}中期望标识符、逗号、`[...]` 列表或 `-` 排除项，得到 `{}`",
                    what,
                    tokens[i]
                ));
            }
        }
    }
    if prev_was_comma {
        return Err(compile_err!(
            "batch-impl: {}中逗号位置不合法（不允许前导/尾随/连续逗号）",
            what
        ));
    }
    let names: Vec<Ident> =
        keep.into_iter().filter(|id| !exclude.iter().any(|e| e == id)).collect();
    if names.is_empty() {
        return Err(compile_err!("batch-impl: {}不能为空", what));
    }
    Ok(names)
}

/// `-` 后的目标：标识符（`-foo`）或 `@all` 系列标记（`-@all_methods`）。
/// 返回（展开的 item 名列表, 消费的 token 数）。
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
            "batch-impl: {}中 `-` 后期望标识符或 `[...]` 列表（如 `-foo`、`-[a,b]`）",
            what
        )),
    }
}

/// `all` 系标记 → (include_fn, include_const, include_type, default 过滤)。
/// `default=None` 全含；`Some(true)` 仅默认实现；`Some(false)` 仅无默认（required）。
/// 指令域（`@all`）与宏元层（`@all`）共用同一张表。
pub(crate) fn resolve_all_marker(
    name: &str,
) -> Option<((bool, bool, bool), Option<bool>)> {
    match name {
        "all" => Some(((true, true, true), None)),
        "all_methods" => Some(((true, false, false), None)),
        "all_constants" => Some(((false, true, false), None)),
        "all_types" => Some(((false, false, true), None)),
        "all_default" => Some(((true, true, true), Some(true))),
        "all_default_methods" => Some(((true, false, false), Some(true))),
        "all_default_constants" => Some(((false, true, false), Some(true))),
        "all_default_types" => Some(((false, false, true), Some(true))),
        "all_required" => Some(((true, true, true), Some(false))),
        "all_required_methods" => Some(((true, false, false), Some(false))),
        "all_required_constants" => Some(((false, true, false), Some(false))),
        "all_required_types" => Some(((false, false, true), Some(false))),
        _ => None,
    }
}

/// 收集 trait item 名。`include_*` 控制种类；`default` 过滤默认实现状态：
/// `Some(true)` 仅含带默认实现的、`Some(false)` 仅含无默认（required）、
/// `None` 全含（syn 的 `default` 字段：fn=默认体、const=默认值、type=默认类型）。
pub(crate) fn get_trait_item_names(
    trait_def: &ItemTrait, include_fn: bool, include_const: bool, include_type: bool,
    default: Option<bool>,
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
        if include && default.is_none_or(|d| d == has_default) {
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
        "batch-impl: trait `{}` 中没有找到 item `{}`",
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
