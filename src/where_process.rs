//! 裸 `where` 新语法预处理。
//!
//! [`where_process`] 在指令预处理之后、DSL 解析之前扫描 token 流中的
//! 裸 `where 谓词 {代码块}` 形式：收集谓词直至首个深度 0 的 `{...}`
//! 代码块（排除 `ident!{...}` 宏调用体与尖括号内代码块），改写为旧式
//! `where{谓词}` 后缀；缺代码块时报 `compile_error!`。三个接口
//! （`#[batch_impl]` / `#[batch_impl_only]` / `batch_trait!`）共用，
//! 解析层无需感知新语法。

use proc_macro2::{Delimiter, Group, TokenStream, TokenTree};

use crate::diagnostic::compile_error_str;
use crate::scan::{Cursor, is_arrow};

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
                if g.delimiter() == Delimiter::Brace)
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
            && g.delimiter() == Delimiter::Bracket
            // `ident![...]` 宏调用体是透传的宏参数，不递归（与 `ident!{...}` 一致）
            && !(i > 0
                && matches!(&tokens[i - 1], TokenTree::Punct(p)
                    if p.as_char() == '!'))
        {
            let v = g.stream().into_iter().collect::<Vec<_>>();
            let vt = where_process(&mut Cursor::new(&v))?;
            result.push(
                Group::new(Delimiter::Bracket, vt.into_iter().collect()).into(),
            );
            i += 1
        } else {
            result.push(tokens[i].clone());
            i += 1;
        };
    }
    Ok(result)
}
/// 谓词区边界 = 首个深度0 {group}，排除 ident!{...}
fn scan_body_boundary(tokens: &[TokenTree]) -> Option<(TokenTree, usize)> {
    let mut depth = 0usize;
    let mut j = 0;
    let mut result = vec![];
    while j < tokens.len() {
        match &tokens[j] {
            TokenTree::Punct(p) if p.as_char() == '<' => {
                depth += 1;
                result.push(&tokens[j])
            }
            TokenTree::Punct(p) if p.as_char() == '>' => {
                if !is_arrow(tokens, j) {
                    depth = depth.saturating_sub(1);
                }
                result.push(&tokens[j])
            }
            TokenTree::Group(g)
                if g.delimiter() == Delimiter::Brace
                    && depth == 0
                    && !is_macro_body(tokens, j) =>
            {
                return (
                    Group::new(
                        Delimiter::Brace,
                        result.into_iter().cloned().collect(),
                    )
                    .into(),
                    j,
                )
                    .into();
            }
            TokenTree::Ident(w) if w == "where" && depth == 0 => {
                return (
                    Group::new(
                        Delimiter::Brace,
                        result.into_iter().cloned().collect(),
                    )
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
