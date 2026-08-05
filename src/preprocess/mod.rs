//! 预处理层：token 重写器（一个趟一个文件）。
//!
//! - [`angle`]：`<>` 配对为尖括号组（入口转换）；
//! - [`consts`]：`@` 常量展开（宏元层，词法替换）；
//! - [`mod`](self)：`#` 指令展开（fill/delegate/blanket/开放扩展）；
//! - [`where_process`]：裸 `where` 谓词改写；
//! - [`empty_generics`]：`A<>` 照抄；
//! - [`helpers`]：指令参数解析辅助。
//!
//! 各趟按固定顺序由 entry 层调用；`mod.rs` 聚合 re-export，
//! 引用侧写 `crate::preprocess::X`。

// ============================================================
// 分隔符拼写宏
// ============================================================

/// 分隔符拼写宏：统一 `Delimiter::*` 字面量为源码分隔符拼写
/// （调用统一用 `[]`）——`delimiter![{}]` / `delimiter![[]]` /
/// `delimiter![()]` 与源码一一对应。
///
/// proc-macro2 的 `Delimiter` 无"尖括号"变体，`<>` 必须借用 `Delimiter::None`
/// 承载——而 `None` 本身也是真实"透明组"的拼写。为避免两义，宏用两种拼写
/// 区分：
/// - `delimiter![<>]`：**尖括号组**载体（`angle_collect` 配对产物）；
/// - `delimiter![none]`：**真实透明组**（宏变量 `$var:ty` 展开产物，
///   内容即 DSL token，需扁平化）。
///
/// 二者展开值相同（`Delimiter::None`），不可在同一条 `match` 中作两个臂
/// （会报 unreachable pattern）；实际用法分布在互斥的上下文，无冲突。
macro_rules! delimiter {
    ({}) => {
        ::proc_macro2::Delimiter::Brace
    };
    ([]) => {
        ::proc_macro2::Delimiter::Bracket
    };
    (()) => {
        ::proc_macro2::Delimiter::Parenthesis
    };
    (<>) => {
        ::proc_macro2::Delimiter::None
    };
    (none) => {
        ::proc_macro2::Delimiter::None
    };
}

pub(crate) mod angle;
pub(crate) mod consts;
pub(crate) mod consts_ctx;
pub(crate) mod empty_generics;
pub(crate) mod helpers;
pub(crate) mod where_process;

pub(crate) use angle::*;
pub(crate) use consts::*;
pub(crate) use consts_ctx::*;
pub(crate) use empty_generics::*;
pub(crate) use helpers::*;
pub(crate) use where_process::*;

mod blanket;
pub(crate) use blanket::expand_blanket;

use proc_macro2::{Group, Ident, TokenStream, TokenTree};
use quote::quote;
use syn::ItemTrait;

use crate::util::Cursor;
use crate::util::{compile_err, compile_error_str};

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
/// | 覆盖 | `#blanket(args){包装列表}` | 多段完整 spec（见 [`expand_blanket`]） |
///
/// 展开产物：既有指令恰为一个 `{...}` 组（可附着到类型或独立成 spec）；
/// `#blanket` 产出多段 spec，只能独立（自含泛型/目标/委托，见
/// architecture.md「语法域隔离」的附着语义说明）。
///
/// `args` 中出现 `@all` 表示 trait 的所有 item（fn + const + type），
/// `@all_methods` 仅 Fn 方法，`@all_constants` 仅 const，`@all_types` 仅 type。
///
/// ## 递归规则
///
/// 只递归展开 `[...]`（Bracket）Group 内容；`(...)` 和 `{...}` 不递归，
/// 避免误入指令的参数或 body。
pub(crate) fn expand_tokens(
    cursor: &mut Cursor, trait_def: &ItemTrait, trait_full_path: &TokenStream,
) -> Result<Vec<TokenTree>, TokenStream> {
    let mut result = vec![];
    while !cursor.at_end() {
        if cursor.is_punct('#')
            && let Some(TokenTree::Ident(name)) = cursor.peek_at(1)
        {
            result.extend(expand_directive(
                name,
                cursor,
                trait_def,
                trait_full_path,
            )?);
            continue;
        }
        // 循环条件保证非 at_end，break 仅为防御
        let Some(tt) = cursor.peek() else {
            break;
        };
        // 只递归展开 [...]（`ident![...]` / `#[...]` 透传，与 angle_collect 守卫对齐）
        if let TokenTree::Group(g) = tt
            && g.delimiter() == delimiter![[]]
            && !cursor.prev_bracket_passthrough()
        {
            let inner = expand_tokens(
                &mut Cursor::new(&g.stream().into_iter().collect::<Vec<_>>()),
                trait_def,
                trait_full_path,
            )?;
            let new_group = Group::new(g.delimiter(), inner.into_iter().collect());
            result.push(new_group.into());
            cursor.bump();
        } else {
            result.push(tt.clone());
            cursor.bump();
        }
    }
    Ok(result)
}

/// 分派到各展开函数，产物约定见 [`expand_tokens`]。
fn expand_directive(
    name: &Ident, cursor: &mut Cursor, trait_def: &ItemTrait,
    trait_full_path: &TokenStream,
) -> Result<Vec<TokenTree>, TokenStream> {
    if let Some(TokenTree::Group(args)) = cursor.peek_at(2) {
        match args.delimiter() {
            delimiter![{}] => {
                // `#name{body}` — item 名紧跟 `{body}`（fn / const / type 通用）
                cursor.bump(); // #
                cursor.bump(); // method_name
                cursor.bump(); // {body}
                expand_single(name, args, trait_def).map(|tt| vec![tt])
            }
            _ => {
                // `#cmd(args){body}` — 名称 + 括号参数 + {body}
                let body_tt = cursor.peek_at(3);
                let Some(TokenTree::Group(body)) = body_tt else {
                    return Err(compile_err!(
                        "`#{}` 后期望 `(args)` + `{{body}}` 或直接 `{{body}}`",
                        name
                    ));
                };
                if body.delimiter() != delimiter![{}] {
                    return Err(compile_err!(
                        "`#{}` 后期望 `(args)` + `{{body}}` 或直接 `{{body}}`",
                        name
                    ));
                }
                cursor.bump(); // #
                cursor.bump(); // name
                cursor.bump(); // (args)
                cursor.bump(); // {body}
                match name.to_string().as_str() {
                    "fill" => expand_fill(args, body, trait_def).map(|tt| vec![tt]),
                    "delegate" => {
                        expand_delegate(args, body, trait_def).map(|tt| vec![tt])
                    }
                    "blanket" => {
                        expand_blanket(args, body, trait_def, trait_full_path)
                    }
                    // 开放扩展：`#name(args){body}` → `{ name!{(args){body} trait_def} }`
                    // 一个函数式宏调用，位于 impl body（附着用法）或顶层（独立用法）。
                    // 与 `#fill`/`#delegate` 同源：把"读 trait → 生成 fn 定义"的实现
                    // 交给用户的同名宏——它解析 args / body / trait 并生成 impl 项。
                    _ => {
                        let inner = quote! {
                            #name ! { #args #body #trait_def }
                        };
                        Ok(vec![Group::new(delimiter![{}], inner).into()])
                    }
                }
            }
        }
    } else {
        Err(compile_err!(
            "`#{}` 后期望括号参数 `(args)` 或代码块 `{{body}}`",
            name
        ))
    }
}

/// `#name{body}` 展开为对应该 item 类型的实现体（见上表）。
///
/// 根据 `name` 在 trait 定义中查找对应的 item，由 `build_from_item` 按 item 类型自动输出。
fn expand_single(
    method_name: &Ident, body: &Group, trait_def: &ItemTrait,
) -> Result<TokenTree, TokenStream> {
    let item = get_trait_item(trait_def, method_name)?;
    Ok(Group::new(delimiter![{}], build_from_item(item, &body.stream())).into())
}

/// 多 item 指令展开的公共骨架：解析方法名列表 → 逐 item 构造实现 → 打包为 `{...}` 组。
/// `build` 按 item 构造实现体（可报错，如 `#delegate` 的非 fn 项/解构参数）。
fn expand_many(
    args_group: &Group, trait_def: &ItemTrait,
    build: impl Fn(&Ident, &syn::TraitItem) -> Result<TokenStream, TokenStream>,
) -> Result<TokenTree, TokenStream> {
    let method_names = parse_names_from_tokens(
        &args_group.stream().into_iter().collect::<Vec<_>>(),
        trait_def,
    )?;
    let mut methods = TokenStream::new();
    for name in &method_names {
        let item = get_trait_item(trait_def, name)?;
        methods.extend(build(name, item)?);
    }
    Ok(Group::new(delimiter![{}], methods).into())
}

/// `#fill(args){body}` → `{fn m1(sig){body} fn m2(sig){body} ...}`
///
/// `args` 为逗号分隔的 item 名列表，或 `@all`（表示所有 item）。
/// 支持 fn、const、type 三种 item 类型。
/// 为每个 item 从 trait 定义读取签名/类型，body 作为实现体。
fn expand_fill(
    args_group: &Group, body: &Group, trait_def: &ItemTrait,
) -> Result<TokenTree, TokenStream> {
    let body_stream = body.stream();
    expand_many(args_group, trait_def, |_name, item| {
        Ok(build_from_item(item, &body_stream))
    })
}

/// `#delegate(args){target}` → `{fn m1(sig){(target).m1(params)} ...}`
///
/// 为每个方法生成委托调用：跳过 `self` 参数，将其余参数原样转发。
fn expand_delegate(
    args_group: &Group, target: &Group, trait_def: &ItemTrait,
) -> Result<TokenTree, TokenStream> {
    let target_stream = target.stream();
    expand_many(args_group, trait_def, |name, item| {
        let syn::TraitItem::Fn(f) = item else {
            return Err(compile_err!(
                "batch-impl: #delegate 只能用于方法，trait `{}` 中的 `{}` 不是方法",
                trait_def.ident,
                name
            ));
        };
        let sig = f.sig.clone();
        let call_args = collect_call_args(&sig).map_err(|pat| {
            compile_err!(
                "batch-impl: #delegate 方法 `{}::{}` 的参数 `{}` 无法委托转发：\
                 仅支持 `self` 与纯标识符模式",
                trait_def.ident,
                name,
                pat
            )
        })?;
        let body = quote! { (#target_stream) . #name ( #(#call_args),* ) };
        Ok(build_from_item(item, &body))
    })
}
