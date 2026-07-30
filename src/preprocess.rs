//! 指令预处理。
//!
//! `#[batch_impl]` 在 DSL 解析前通过 [`expand_tokens`] 扫描 `#` 指令，
//! 从 trait 定义自动读取 fn / const / type item 的签名，
//! 生成等价 `{ ... }` 代码块替换到原 token 流中。内置指令
//! `#name{body}` / `#fill(args){body}` / `#delegate(args){target}`
//! 在本模块处理；不认识的 `#name` 自动委托给 Rust 的属性宏系统。
//!
//! v0.4.2：诊断改为统一通过 [`crate::diagnostic::compile_error_str`]
//! 构造；`expand_tokens` 内 `unwrap` 全部消除，预处理层零 panic 点。

use proc_macro2::{Delimiter, Group, Ident, TokenStream, TokenTree};
use quote::quote;
use syn::ItemTrait;

use crate::diagnostic::compile_error_str;
use crate::preprocess_helpers::{build_from_item, collect_call_args, get_trait_item, parse_names_from_tokens};
use crate::scan::Cursor;

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
/// | 单 item | `#name{body}` | `{fn method(签名) { body }}` 或 `{const NAME: Type = body;}` 或 `{type Name = body;}` |
/// | 填充 | `#fill(args){body}` | `{fn m1(sig){body} fn m2(sig){body} ...}` |
/// | 委托 | `#delegate(args){target}` | `{fn m1(sig){(target).m1(args)} ...}` |
///
/// `#name{body}` 中 `name` 可以是 trait 中的任意 item 名（方法、常量、类型），
/// `build_from_item` 根据 item 类型自动选择输出格式。
///
/// `args` 中出现 `#all` 表示 trait 的所有 item（fn + const + type），
/// `#all_methods` 仅 Fn 方法，`#all_constants` 仅 const，`#all_types` 仅 type。
///
/// ## 递归规则
///
/// 只递归展开 `[...]`（Bracket）Group 内容；`(...)` 和 `{...}` 不递归，
/// 避免误入指令的参数或 body。
pub(crate) fn expand_tokens(
    cursor: &mut Cursor,
    trait_def: &ItemTrait,
) -> Result<Vec<TokenTree>, TokenStream> {
    let mut result = vec![];
    while !cursor.at_end() {
        if cursor.is_punct('#')
            && let Some(TokenTree::Ident(name)) = cursor.peek_at(1)
        {
            let expanded = expand_directive(name, cursor, trait_def)?;
            result.extend(expanded);
            continue;
        }
        // 当前 token 一定存在（循环条件保证了非 at_end）
        let Some(tt) = cursor.peek() else {
            // 逻辑上不可达；防御性 break 以兜底
            break;
        };
        // 只递归展开 [...] 内容（`(...)` 和 `{...}` 不递归）
        if let TokenTree::Group(g) = tt
            && g.delimiter() == Delimiter::Bracket
        {
            let inner = expand_tokens(
                &mut Cursor::new(
                    &g.stream().into_iter().collect::<Vec<_>>(),
                ),
                trait_def,
            )?;
            let new_group =
                Group::new(g.delimiter(), inner.into_iter().collect());
            result.push(new_group.into());
            cursor.bump();
        } else {
            result.push(tt.clone());
            cursor.bump();
        }
    }
    Ok(result)
}

/// 分派指令：根据 `#` 后的名称和括号结构分派到对应的展开函数。
fn expand_directive(
    name: &Ident,
    cursor: &mut Cursor,
    trait_def: &ItemTrait,
) -> Result<Vec<TokenTree>, TokenStream> {
    if let Some(TokenTree::Group(args)) = cursor.peek_at(2) {
        match args.delimiter() {
            Delimiter::Brace => {
                // `#name{body}` — item 名紧跟 `{body}`（fn / const / type 通用）
                cursor.bump(); // #
                cursor.bump(); // method_name
                cursor.bump(); // {body}
                expand_single(name, args, trait_def)
            },
            _ => {
                // `#cmd(args){body}` — 名称 + 括号参数 + {body}
                let body_tt = cursor.peek_at(3);
                let Some(TokenTree::Group(body)) = body_tt else {
                    return Err(compile_error_str(&format!(
                        "`#{}` 后期望 `(args)` + `{{body}}` 或直接 `{{body}}`",
                        name
                    )));
                };
                if body.delimiter() != Delimiter::Brace {
                    return Err(compile_error_str(&format!(
                        "`#{}` 后期望 `(args)` + `{{body}}` 或直接 `{{body}}`",
                        name
                    )));
                }
                cursor.bump(); // #
                cursor.bump(); // name
                cursor.bump(); // (args)
                cursor.bump(); // {body}
                match name.to_string().as_str() {
                    "fill" => expand_fill(args, body, trait_def),
                    "delegate" => expand_delegate(args, body, trait_def),
                    _ => Ok(quote! {
                        #[#name[#args #body]]#trait_def
                    }
                    .into_iter()
                    .collect()),
                }
            },
        }
    } else {
        Err(compile_error_str(&format!(
            "`#{}` 后期望括号参数 `(args)` 或代码块 `{{body}}`",
            name
        )))
    }
}

/// `#name{body}` → `{fn method(签名) { body }}` 或 `{const NAME: Type = body;}` 或 `{type Name = body;}`
///
/// 根据 `name` 在 trait 定义中查找对应的 item，由 `build_from_item` 按 item 类型自动输出。
fn expand_single(
    method_name: &Ident,
    body: &Group,
    trait_def: &ItemTrait,
) -> Result<Vec<TokenTree>, TokenStream> {
    let item = get_trait_item(trait_def, method_name)?;
    Ok(vec![TokenTree::Group(Group::new(
        Delimiter::Brace,
        build_from_item(item, &body.stream()),
    ))])
}

/// `#fill(args){body}` → `{fn m1(sig){body} fn m2(sig){body} ...}`
///
/// `args` 为逗号分隔的 item 名列表，或 `#all`（表示所有 item）。
/// 支持 fn、const、type 三种 item 类型。
/// 为每个 item 从 trait 定义读取签名/类型，body 作为实现体。
fn expand_fill(
    args_group: &Group,
    body: &Group,
    trait_def: &ItemTrait,
) -> Result<Vec<TokenTree>, TokenStream> {
    let method_names = parse_names_from_tokens(
        &args_group.stream().into_iter().collect::<Vec<_>>(),
        trait_def,
    )?;
    let mut methods = TokenStream::new();
    for name in &method_names {
        let item = get_trait_item(trait_def, name)?;
        methods.extend(build_from_item(&item, &body.stream()));
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
    args_group: &Group,
    target: &Group,
    trait_def: &ItemTrait,
) -> Result<Vec<TokenTree>, TokenStream> {
    let method_names = parse_names_from_tokens(
        &args_group.stream().into_iter().collect::<Vec<_>>(),
        trait_def,
    )?;
    let mut methods = TokenStream::new();
    for name in &method_names {
        let item = get_trait_item(trait_def, name)?;
        let syn::TraitItem::Fn(f) = item else {
            return Err(compile_error_str(&format!(
                "batch-impl: #delegate 只能用于方法，trait `{}` 中的 `{}` 不是方法",
                trait_def.ident, name
            )));
        };
        let sig = f.sig.clone();
        let call_args = collect_call_args(&sig);
        let target = target.stream();
        let body = quote! { (#target) . #name ( #(#call_args),* ) };
        methods.extend(build_from_item(&item, &body));
    }
    Ok(vec![TokenTree::Group(Group::new(
        Delimiter::Brace,
        methods,
    ))])
}
