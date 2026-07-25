use proc_macro2::{Delimiter, Group, Ident, TokenStream, TokenTree};
use quote::{quote, ToTokens};
use syn::ItemTrait;

use crate::parse::Cursor;

// ============================================================
// 指令预处理
// ============================================================

/// 指令预处理入口：扫描 token 流，展开 `#` 指令。
///
/// 仅 `#[batch_impl]` / `#[batch_impl_only]` 支持（需要 trait 定义读取方法签名）。
/// `batch_trait!` 不调用此函数（无 trait 定义可用）。
///
/// ## 指令语法
///
/// | 指令 | 语法 | 效果 |
/// |------|------|------|
/// | 单方法 | `#method{body}` | `{fn method(签名) { body }}` |
/// | 填充 | `#fill(args){body}` | `{fn m1(sig){body} fn m2(sig){body} ...}` |
/// | 委托 | `#delegate(args){target}` | `{fn m1(sig){(target).m1(args)} ...}` |
///
/// `args` 中出现 `#all` 表示 trait 的所有方法。
///
/// ## 递归规则
///
/// 只递归展开 `[...]`（Bracket）Group 内容；`(...)` 和 `{...}` 不递归，
/// 避免误入指令的参数或 body。
pub(crate) fn expand_tokens(cursor: &mut Cursor, trait_def: &ItemTrait) -> Result<Vec<TokenTree>, TokenStream> {
    let mut result = vec![];
    while !cursor.at_end() {
        if cursor.is_punct('#')
            && let Some(TokenTree::Ident(name))=cursor.peek_at(1) {
            let expanded = expand_directive(name, cursor, trait_def)?;
            result.extend(expanded);
            continue;
        }
        // 只递归展开 [...] 内容（`(...)` 和 `{...}` 不递归）
        if let TokenTree::Group(g) = cursor.peek().unwrap() &&
            g.delimiter()==Delimiter::Bracket{
            let inner =
                expand_tokens(&mut Cursor::new(&g.stream().into_iter().collect::<Vec<_>>()), trait_def)?;
            let new_group = Group::new(g.delimiter(), inner.into_iter().collect());
            result.push(new_group.into());
            cursor.bump();
        } else {
            result.push(cursor.peek().unwrap().clone());
            cursor.bump();
        }
    }
    Ok(result)
}

/// 分派指令：根据 `#` 后的名称和括号结构分派到对应的展开函数。
fn expand_directive(
    name:&Ident,
    cursor: &mut Cursor,
    trait_def: &ItemTrait,
) -> Result<Vec<TokenTree>, TokenStream> {
    if let Some(TokenTree::Group(args))=cursor.peek_at(2){
        if args.delimiter()==Delimiter::Brace{
            // `#method{body}` — 方法名紧跟 `{body}`
            cursor.bump(); // #
            cursor.bump(); // method_name
            cursor.bump(); // {body}
            expand_single_method(name, args, trait_def)
        }else if let Some(TokenTree::Group(body))=cursor.peek_at(3)&&
            body.delimiter()==Delimiter::Brace{
            // `#cmd(args){body}` — 名称 + 括号参数 + {body}
            cursor.bump(); // #
            cursor.bump(); // name
            cursor.bump(); // (args)
            cursor.bump(); // {body}
            match name.to_string().as_str(){
                "fill"=> expand_fill(args, body, trait_def),
                "delegate"=> expand_delegate(args, body, trait_def),
                _=> Ok(quote!{
                        #[#name[#args #body]]#trait_def
                    }.into_iter().collect())
            }
        }else{
            Err(compile_error(&format!(
                "`#{}` 后期望 `(args)` + `{{body}}` 或直接 `{{body}}`",
                name
            )))
        }
    }else{
        Err(compile_error(&format!(
            "`#{}` 后期望括号参数 `(args)` 或代码块 `{{body}}`",
            name
        )))
    }
}

/// `#method{body}` → `{fn method(trait 中的签名) { body }}`
fn expand_single_method(
    method_name:&Ident,
    body:&Group,
    trait_def: &ItemTrait,
) -> Result<Vec<TokenTree>, TokenStream> {
    let sig = get_trait_method_sig(trait_def, method_name)?;
    Ok(vec![TokenTree::Group(Group::new(
        Delimiter::Brace,
        build_fn_from_sig(&sig, &body.stream()),
    ))])
}

/// `#fill(args){body}` → `{fn m1(sig){body} fn m2(sig){body} ...}`
///
/// `args` 为逗号分隔的方法名列表，或 `#all`（表示所有方法）。
/// 为每个方法从 trait 定义读取签名，body 作为实现体。
fn expand_fill(
    args_group:&Group,
    body:&Group,
    trait_def: &ItemTrait,
) -> Result<Vec<TokenTree>, TokenStream> {
    let method_names = parse_method_names_from_tokens(&args_group.stream().into_iter().collect::<Vec<_>>(), trait_def)?;

    let mut methods = TokenStream::new();
    for name in &method_names {
        let sig=get_trait_method_sig(trait_def, name)?;
        methods.extend(build_fn_from_sig(&sig, &body.stream()));
    }
    Ok(vec![TokenTree::Group(Group::new(
        Delimiter::Brace,
        methods,
    ))])
}

/// `#delegate(args){target}` → `{fn m1(sig){(target).m1(params)} ...}`
///
/// 为每个方法生成委托调用：跳过 `self` 参数，将其余参数原样转发。
fn expand_delegate(
    args_group:&Group,
    target:&Group,
    trait_def: &ItemTrait,
) -> Result<Vec<TokenTree>, TokenStream> {
    let method_names = parse_method_names_from_tokens(&args_group.stream().into_iter().collect::<Vec<_>>(), trait_def)?;
    let mut methods = TokenStream::new();
    for name in &method_names {
        let sig=get_trait_method_sig(trait_def, name)?;
        let call_args = collect_call_args(&sig);
        let target=target.stream();
        let body = quote! { (#target) . #name ( #(#call_args),* ) };
        methods.extend(build_fn_from_sig(&sig, &body));
    }
    Ok(vec![TokenTree::Group(Group::new(
        Delimiter::Brace,
        methods,
    ))])
}

// ============================================================
// 辅助函数
// ============================================================

/// 从 token 序列解析方法名列表：逗号分隔的标识符，或 `#all`。
fn parse_method_names_from_tokens(
    tokens: &[TokenTree],
    trait_def: &ItemTrait,
) -> Result<Vec<Ident>, TokenStream> {
    if tokens.is_empty() {
        return Err(compile_error("batch-impl: 指令的参数列表不能为空"));
    }
    // `#all` 特殊标记
    if tokens.len() == 2
        && let (TokenTree::Punct(p), TokenTree::Ident(id)) = (&tokens[0], &tokens[1])
        && p.as_char() == '#' && id == "all"
    {
        return Ok(get_all_trait_methods(trait_def));
    }
    tokens
        .iter()
        .map(|t| {
            if let TokenTree::Ident(id) = t {
                Ok(Ident::new(&id.to_string(), id.span()))
            } else if let TokenTree::Punct(p) = t &&
                p.as_char()==','{
                Err(None)
            }else {
                Err(Some(compile_error(&format!(
                    "batch-impl: 指令参数中期望标识符或逗号，得到 `{}`",
                    t
                ))))
            }
        })
        .filter_map(|r| match r {
            Ok(v) => Some(Ok(v)),
            Err(None)  => None,
            Err(Some(e)) => Some(Err(e)),
        })
        .collect::<Result<Vec<_>, _>>()
}

/// 获取 trait 定义中的所有方法名
fn get_all_trait_methods(trait_def: &ItemTrait) -> Vec<Ident> {
    trait_def
        .items
        .iter()
        .filter_map(|item| {
            if let syn::TraitItem::Fn(f) = item {
                Some(f.sig.ident.clone())
            } else {
                None
            }
        })
        .collect()
}

/// 从 trait 定义中查找指定方法的签名
fn get_trait_method_sig(trait_def: &ItemTrait, name: &Ident) -> Result<syn::Signature, TokenStream> {
    for item in &trait_def.items{
        if let syn::TraitItem::Fn(f) = item && f.sig.ident == *name {
            return Ok(f.sig.clone());
        }
    }
    Err(compile_error(&format!(
        "batch-impl: trait `{}` 中没有找到方法 `{}`",
        trait_def.ident, name
    )))
}

/// 用方法签名 + body 构建完整的 fn token 流
fn build_fn_from_sig(sig: &syn::Signature, body: &TokenStream) -> TokenStream {
    let sig_tokens = sig.to_token_stream();
    quote! { #sig_tokens { #body } }
}

/// 从方法签名中收集调用参数（跳过 self，取其余参数名）
fn collect_call_args(sig: &syn::Signature) -> Vec<Ident> {
    sig.inputs
        .iter()
        .filter_map(|arg| {
            if let syn::FnArg::Typed(pat_type) = arg
                && let syn::Pat::Ident(pat_ident) = &*pat_type.pat
            {
                return Some(pat_ident.ident.clone());
            }
            None
        })
        .collect()
}

fn compile_error(msg: &str) -> TokenStream {
    quote! { compile_error!(#msg); }
}
