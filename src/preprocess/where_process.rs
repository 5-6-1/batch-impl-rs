//! 裸 `where` 新语法预处理。
//!
//! [`where_process`] 在指令预处理之后、DSL 解析之前扫描 token 流中的
//! 裸 `where 谓词 {代码块}` 形式：收集谓词直至首个深度 0 的 `{...}`
//! 代码块（排除 `ident!{...}` 宏调用体与尖括号内代码块），改写为旧式
//! `where{谓词}` 后缀；缺代码块时报 `compile_error!`。三个接口
//! （`#[batch_impl]` / `#[batch_impl_only]` / `batch_trait!`）共用，
//! 解析层无需感知新语法。
//!
//! **限制**：谓词区边界只按 `<>` 深度扫描，不跟踪 `()`/`[]` 深度——但
//! proc-macro2 会把平衡的 `(...)`/`[...]` 聚合成单个 Group token（对扫描不透明），
//! 因此 `Fn({code})` 这类括号内代码块不会误判为 body 边界；仅**不平衡**的
//! 括号（本就是非法输入）才可能受影响。

use proc_macro2::{Group, TokenStream, TokenTree};

use crate::diagnostic::compile_error_str;
use crate::scan::Cursor;

pub(crate) fn where_process(
    cursor: &mut Cursor,
) -> Result<Vec<TokenTree>, TokenStream> {
    let tokens = cursor.take_rest();
    let mut result = vec![];
    let mut i = 0;
    while i < tokens.len() {
        // 裸 `where`：后紧跟 {group} 是旧式 `where{...}`，原样跳过；
        // 否则收集谓词区到边界（首个深度 0 的 {group}，排除 ident!{...}），
        // 包成 where{谓词} 推入 result，i 跳到边界处（body 由下轮原样复制）
        if let TokenTree::Ident(ident) = &tokens[i]
            && ident == "where"
            && i + 1 < tokens.len()
            && !matches!(&tokens[i+1],TokenTree::Group(g)
                if g.delimiter() == delimiter![{}])
        {
            let Some((where_body, rest_index)) = scan_body_boundary(&tokens[i + 1..])
            else {
                return Err(compile_error_str(
                    "batch-impl: `where` 谓词后缺少代码块 {...}",
                ));
            };
            result.push(ident.clone().into());
            result.push(where_body);
            i += 1 + rest_index;
        } else if let TokenTree::Group(g) = &tokens[i]
            && g.delimiter() == delimiter![[]]
            // `ident![...]` 宏调用体是透传的宏参数，不递归（与 `ident!{...}` 一致）
            && !(i > 0
                && matches!(&tokens[i - 1], TokenTree::Punct(p)
                    if p.as_char() == '!'))
        {
            let v = g.stream().into_iter().collect::<Vec<_>>();
            let vt = where_process(&mut Cursor::new(&v))?;
            result.push(Group::new(delimiter![[]], vt.into_iter().collect()).into());
            i += 1
        } else {
            result.push(tokens[i].clone());
            i += 1;
        };
    }
    Ok(result)
}
/// 谓词区边界 = 首个 `{...}` 组（排除 `ident!{...}` 宏体）。
/// 尖括号已由 `angle_collect` 配对为不透明组，无需再跟踪 `<>` 深度。
fn scan_body_boundary(tokens: &[TokenTree]) -> Option<(TokenTree, usize)> {
    let mut j = 0;
    let mut result = vec![];
    while j < tokens.len() {
        match &tokens[j] {
            TokenTree::Group(g)
                if g.delimiter() == delimiter![{}] && !is_macro_body(tokens, j) =>
            {
                return (
                    Group::new(delimiter![{}], result.into_iter().cloned().collect())
                        .into(),
                    j,
                )
                    .into();
            }
            TokenTree::Ident(w) if w == "where" => {
                return (
                    Group::new(delimiter![{}], result.into_iter().cloned().collect())
                        .into(),
                    j,
                )
                    .into();
            }
            _ => result.push(&tokens[j]),
        }
        j += 1;
    }
    None
}

fn is_macro_body(tokens: &[TokenTree], index: usize) -> bool {
    index >= 2
        && matches!(&tokens[index - 1], TokenTree::Punct(p) if p.as_char() == '!')
        && matches!(&tokens[index - 2], TokenTree::Ident(_))
}
