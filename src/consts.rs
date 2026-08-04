//! `@` 常量系统：内置类型族常量 + 用户自定义常量的展开。
//!
//! 语法（token 流层面）：
//! - **名字族**：`@uint` / `@int` / `@float` / `@num` / `@scalar`
//! - **范围族**：`@u8..u128` / `@i8..i128` / `@f32..f64`（含端点；宽度校验）
//! - **用户定义**（仅 `batch_trait!`）：前导 `@name=值;` 段，值为任意 DSL
//!   表达式（可引用内置常量；引用已定义的用户常量按定义顺序自然可用）
//!
//! 展开产物是 Bracket 列表（`[u8, u16, ...]`），与手写列表逐 token 等价，
//! 走原管线。宏元层（architecture.md「语法域隔离」）：只做词法替换，
//! 不参与任何域内解析。
//!
//! 管线位置：`angle_collect` 之后（定义段值里的 `<>` 已配对为组）、指令
//! 预处理之前（展开产物若含 `#name` 指令仍会被正常处理）。

use proc_macro2::{Group, Ident, Span, TokenStream, TokenTree};
use quote::quote;
use std::collections::HashMap;

use crate::diagnostic::compile_error_str;
use crate::scan::bracket_is_passthrough;

/// 内置名字族：`@name` → 类型标识符列表。
fn builtin_named(name: &str) -> Option<Vec<&'static str>> {
    match name {
        "uint" => Some(vec!["u8", "u16", "u32", "u64", "u128", "usize"]),
        "int" => Some(vec!["i8", "i16", "i32", "i64", "i128", "isize"]),
        "float" => Some(vec!["f32", "f64"]),
        "num" => Some(vec![
            "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64",
            "i128", "isize", "f32", "f64",
        ]),
        "scalar" => Some(vec![
            "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64",
            "i128", "isize", "f32", "f64", "bool", "char",
        ]),
        _ => None,
    }
}

/// 解析范围族端点（`u8` / `i32` / `f64`）为（族, 宽度）。
/// 宽度非法（如 `u9`、`f8`）返回 `None`（族已匹配但宽度不在合法集）。
fn split_range_endpoint(s: &str) -> Option<(char, u32)> {
    let (fam, width_str) = s.split_at(1);
    let fam = fam.chars().next()?;
    let width: u32 = width_str.parse().ok()?;
    let legal: &[u32] = match fam {
        'u' | 'i' => &[8, 16, 32, 64, 128],
        'f' => &[32, 64],
        _ => return None,
    };
    legal.contains(&width).then_some((fam, width))
}

/// 内置范围族：`@u8..u128`（含端点）→ 按宽度升序的类型列表。
/// 端点族不一致或起点大于终点返回 `Err`（调用方构造诊断）。
fn builtin_range(start: &str, end: &str) -> Result<Vec<String>, String> {
    let Some((fam1, w1)) = split_range_endpoint(start) else {
        return Err(format!(
            "`@{}` 宽度非法（合法：u/i 为 8/16/32/64/128，f 为 32/64）",
            start
        ));
    };
    let Some((fam2, w2)) = split_range_endpoint(end) else {
        return Err(format!(
            "`@{}` 宽度非法（合法：u/i 为 8/16/32/64/128，f 为 32/64）",
            end
        ));
    };
    if fam1 != fam2 {
        return Err(format!("范围端点族不一致：`{}` 与 `{}`", start, end));
    }
    if w1 > w2 {
        return Err(format!("范围起点大于终点：`{}..{}`", start, end));
    }
    let widths: &[u32] = match fam1 {
        'u' | 'i' => &[8, 16, 32, 64, 128],
        _ => &[32, 64],
    };
    Ok(widths
        .iter()
        .filter(|w| **w >= w1 && **w <= w2)
        .map(|w| format!("{}{}", fam1, w))
        .collect())
}

/// 把类型名列表渲染为 Bracket 列表组（`[u8, u16, ...]`）。
fn render_list<'a>(names: impl IntoIterator<Item = &'a str>) -> TokenTree {
    let idents: Vec<Ident> =
        names.into_iter().map(|s| Ident::new(s, Span::call_site())).collect();
    Group::new(delimiter![[]], quote!(#(#idents),*)).into()
}

/// 识别并展开 `tokens[i]` 处的 `@` 常量引用；返回（展开产物, 消费的 token 数）。
///
/// 形态（`@` 为 `tokens[i]`）：
/// - `@` Ident `=` … → 用户定义段（仅 `collect_user_consts` 前导收集期出现；
///   此处视为错误——batch_impl 不支持自定义常量）
/// - `@` Ident `..` Ident → 范围族
/// - `@` Ident → 名字族 / 用户表
fn try_expand_at(
    tokens: &[TokenTree], user_table: Option<&HashMap<String, Vec<TokenTree>>>,
) -> Result<(Vec<TokenTree>, usize), TokenStream> {
    let Some(TokenTree::Ident(name)) = tokens.get(1) else {
        return Err(compile_error_str(
            "batch-impl: `@` 后必须跟常量名（如 `@uint`、`@u8..u128`）",
        ));
    };
    let name_str = name.to_string();
    // 定义段：`@name=...`。仅 `collect_user_consts` 的前导收集期消费；
    // 此处出现意味着定义段位置错误——按上下文区分诊断：
    // - `batch_trait!`（user_table 非 None）：定义段未位于前导 → 位置错误；
    // - `#[batch_impl]` / `#[batch_impl_only]`（user_table 为 None）：不支持自定义。
    if let Some(TokenTree::Punct(eq)) = tokens.get(2)
        && eq.as_char() == '='
    {
        let msg = if user_table.is_some() {
            format!(
                "batch-impl: 常量定义 `@{}=...` 必须位于 `batch_trait!` 的\
                 所有 trait 段之前（仅前导位置可定义）",
                name_str
            )
        } else {
            "batch-impl: `#[batch_impl]` / `#[batch_impl_only]` 不支持自定义\
             常量定义；自定义常量仅 `batch_trait!` 支持（前导 `@name=值;` 段）"
                .to_string()
        };
        return Err(compile_error_str(&msg));
    }
    // 范围族：`@` Ident `..` Ident（`..` 为 Joint '.' + 任意 '.'，可选 `=`）
    if let Some(TokenTree::Punct(d1)) = tokens.get(2)
        && d1.as_char() == '.'
        && d1.spacing() == proc_macro2::Spacing::Joint
        && let Some(TokenTree::Punct(d2)) = tokens.get(3)
        && d2.as_char() == '.'
    {
        let end_idx = if let Some(TokenTree::Punct(eq)) = tokens.get(4)
            && eq.as_char() == '='
        {
            5
        } else {
            4
        };
        let Some(TokenTree::Ident(end)) = tokens.get(end_idx) else {
            return Err(compile_error_str(&format!(
                "batch-impl: 范围常量 `@{}{}..` 后缺少终点（如 `@u8..u128`）",
                name_str, ".."
            )));
        };
        let types = builtin_range(&name_str, &end.to_string())
            .map_err(|msg| compile_error_str(&format!("batch-impl: {}", msg)))?;
        return Ok((
            vec![render_list(types.iter().map(|s| s.as_str()))],
            end_idx + 1,
        ));
    }
    // 名字族 / 用户表
    if let Some(expanded) = user_table.and_then(|t| t.get(&name_str)) {
        return Ok((expanded.clone(), 2));
    }
    match builtin_named(&name_str) {
        Some(types) => Ok((vec![render_list(types.iter().copied())], 2)),
        None => Err(compile_error_str(&format!(
            "batch-impl: 未知的 @ 常量 `@{}`；内置：`@uint` `@int` `@float` `@num` \
             `@scalar` 与范围 `@u8..u128` `@i8..i128` `@f32..f64`\
             {}",
            name_str,
            if user_table.is_some() {
                "；batch_trait! 用户常量须在引用前定义（定义在其后不生效）"
            } else {
                ""
            }
        ))),
    }
}

/// 校验常量值内的 `@` 引用可见性：每个 `@` 后的常量名必须属于
/// （已定义用户常量 ∪ 内置常量）。循环引用（`@a=@a`）与前向引用（`@a=@b`
/// 且 `@b` 定义在后）在此拦截——懒展开下循环引用会无限递归，且定义处报错
/// 优于使用处报错。递归进所有组（`[Vec<@uint>]` 的 `@uint` 在尖括号组内）。
fn check_value_refs(
    tokens: &[TokenTree], table: &HashMap<String, Vec<TokenTree>>, def_name: &str,
) -> Result<(), TokenStream> {
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            TokenTree::Punct(p) if p.as_char() == '@' => {
                let Some(TokenTree::Ident(name)) = tokens.get(i + 1) else {
                    return Err(compile_error_str(
                        "batch-impl: 常量值中 `@` 后必须跟常量名（如 `@uint`、`@u8..u128`）",
                    ));
                };
                let name_str = name.to_string();
                let known = builtin_named(&name_str).is_some()
                    || split_range_endpoint(&name_str).is_some()
                    || table.contains_key(&name_str);
                if !known {
                    return Err(compile_error_str(&format!(
                        "batch-impl: 常量 `@{}` 引用未知的 `@{}`（未定义或定义在其后；\
                         常量定义内只能引用内置常量或此前已定义的常量）",
                        def_name, name_str
                    )));
                }
                i += 2;
            }
            TokenTree::Group(g) => {
                check_value_refs(
                    &g.stream().into_iter().collect::<Vec<_>>(),
                    table,
                    def_name,
                )?;
                i += 1;
            }
            _ => i += 1,
        }
    }
    Ok(())
}

/// 展开 token 流中的 `@` 常量引用（内置 + 用户表）。
///
/// 递归规则与 `angle_collect` / `where_process` 一致：`Paren`/`Bracket`（非
/// 透传）递归进入；`Brace`（透传代码，body 里 `@` 是模式语法 `x @ pat`）与
/// `ident![...]` / `#[...]`（任意 Rust）不进入。
pub(crate) fn expand_consts(
    tokens: &[TokenTree], user_table: Option<&HashMap<String, Vec<TokenTree>>>,
) -> Result<Vec<TokenTree>, TokenStream> {
    let mut result = vec![];
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            // DSL 容器（元组 / 列表）：递归进入。
            // 注意：没有 `delimiter![none]`（真实透明组）分支——它与本模块
            // 不相关的尖括号组（`delimiter![<>]`）展开值相同，二者不可同臂
            // 区分（architecture.md「关键设计决策」）；且真实 None 组已由
            // `angle_collect` 在入口扁平化，此处永远不会出现。
            TokenTree::Group(g)
                if g.delimiter() == delimiter![()]
                    || g.delimiter() == delimiter![[]] =>
            {
                if g.delimiter() == delimiter![[]]
                    && bracket_is_passthrough(tokens, i)
                {
                    result.push(tokens[i].clone());
                } else {
                    let inner: Vec<_> = g.stream().into_iter().collect();
                    result.push(
                        Group::new(
                            g.delimiter(),
                            expand_consts(&inner, user_table)?.into_iter().collect(),
                        )
                        .into(),
                    );
                }
                i += 1;
            }
            // 透传代码（body）与其余：原样保留
            TokenTree::Punct(p) if p.as_char() == '@' => {
                let (expanded, consumed) = try_expand_at(&tokens[i..], user_table)?;
                // 懒展开：用户常量值存原样 token（可含嵌套 `@` 引用与 DSL 运算），
                // 拼接后递归展开（循环引用已在定义处拦截，递归必然终止）。
                let expanded = expand_consts(&expanded, user_table)?;
                result.extend(expanded);
                i += consumed;
            }
            _ => {
                result.push(tokens[i].clone());
                i += 1;
            }
        }
    }
    Ok(result)
}

/// 收集 `batch_trait!` 前导的用户常量定义段：`@name=值;`（零个或多个）。
///
/// 返回（剩余 token 流, 用户表）。定义段的值做展开后入库（内置常量 +
/// 此前已定义的用户常量按序可用）；与内置常量同名报错。
pub(crate) type UserConsts = HashMap<String, Vec<TokenTree>>;

pub(crate) fn collect_user_consts(
    tokens: &[TokenTree],
) -> Result<(Vec<TokenTree>, UserConsts), TokenStream> {
    let mut i = 0;
    let mut table = UserConsts::new();
    // 定义段形态：`@` Ident `=`（否则不是定义段，停止收集）
    while let Some(TokenTree::Punct(at)) = tokens.get(i) {
        if at.as_char() != '@' {
            break;
        }
        let Some(TokenTree::Ident(name)) = tokens.get(i + 1) else { break };
        let Some(TokenTree::Punct(eq)) = tokens.get(i + 2) else { break };
        if eq.as_char() != '=' {
            break;
        }
        let name_str = name.to_string();
        // 与内置常量重名 → 报错（防意外覆盖）
        if builtin_named(&name_str).is_some() {
            return Err(compile_error_str(&format!(
                "batch-impl: 用户常量 `@{}` 与内置常量重名；请换名",
                name_str
            )));
        }
        // 值：到深度 0 的 `;` 为止（尖括号已由 angle_collect 配对，无深度问题）
        let mut j = i + 3;
        let mut end = None;
        while j < tokens.len() {
            if let TokenTree::Punct(p) = &tokens[j]
                && p.as_char() == ';'
            {
                end = Some(j);
                break;
            }
            j += 1;
        }
        let Some(end) = end else {
            return Err(compile_error_str(&format!(
                "batch-impl: 常量定义 `@{}=...` 缺少结尾 `;`",
                name_str
            )));
        };
        let value: Vec<TokenTree> = tokens[i + 3..end].to_vec();
        // 值形态：**任意 token**（懒展开——原样入库，引用处拼接 + 递归展开；
        // DSL 运算如 `[Box,Rc]^@num` 定义处接受，使用处展开后由 DSL 层求值）。
        // 唯一定义处校验：引用可见性（循环/前向引用在此拦截——懒展开下
        // `@a=@a` 会无限递归，`@a=@b`（@b 在后）定义处报错优于使用处报错）。
        check_value_refs(&value, &table, &name_str)?;
        table.insert(name_str, value);
        i = end + 1;
    }
    Ok((tokens[i..].to_vec(), table))
}
