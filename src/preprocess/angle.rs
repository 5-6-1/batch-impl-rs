//! 尖括号收集预处理。
//!
//! proc-macro2 的 tokenizer 只对 `()`/`[]`/`{}` 分组，`<>` 是扁平 Punct——
//! 本模块在 DSL 解析前把扁平 `<...>` 配对收集为尖括号组
//! （载体是 `delimiter![<>]` = `Delimiter::None`），使下游 parse 层
//! 不再需要 `<>` 深度跟踪。
//!
//! [`angle_collect`] 一趟扫描同时做两件事：
//! - **真实 `None` 组扁平化**：输入（源码 token）本不该有 `None` 组，
//!   它只来自宏变量（`$var:ty`）展开——其内容就是 DSL token，扁平化后
//!   与直接书写等价（内容里的 `<` 会被本趟配对还原）；
//! - **`<...>` 配对**：扁平 `<` 找匹配 `>`（`->` 箭头的 `>` 不参与配对），
//!   内容递归处理（嵌套 `<`、Paren/Bracket 组），结果包为尖括号组。
//!
//! 递归规则：`Paren`/`Bracket` 是 DSL 容器（元组/列表，内含类型表达式）
//! → 递归进入；`Brace` 是透传代码（body，`a < b` 是真实比较）→ 不进入。
//!
//! [`render_angles`] 是输出侧镜像：把尖括号组还原为 `<` + 内容 + `>`
//! 扁平 token（输出里的尖括号组只可能来自本模块配对——输入的真实
//! `None` 组已被 [`angle_collect`] 扁平化）。

use proc_macro2::{Group, TokenStream, TokenTree};

use crate::diagnostic::compile_error_str;
use crate::scan::is_arrow;

/// 入口转换：一趟扫描完成 None 组扁平化与 `<...>` 配对。
///
/// - `Brace` 组（透传代码）不进入；
/// - `Paren` 组（DSL 元组）递归；`Bracket` 组（DSL 列表）递归，
///   但 `ident![...]` 宏体 / `#[...]` 属性**不进入**（内容可能是任意 Rust，
///   含比较 `<`）；
/// - 扁平 `<`/`>` 必须配对（`->` 箭头的 `>` 不参与）；孤立（未配对）报错——
///   这是非法输入，且报错后下游（scan/where/路径扫描）不再需要 `<>` 深度跟踪。
pub(crate) fn angle_collect(
    tokens: &[TokenTree],
) -> Result<Vec<TokenTree>, TokenStream> {
    let mut out = vec![];
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            // 真实 None 组：内容就是 DSL token，扁平化（内容里的 `<` 由本趟配对）
            TokenTree::Group(g) if g.delimiter() == delimiter![none] => {
                let inner: Vec<_> = g.stream().into_iter().collect();
                out.extend(angle_collect(&inner)?);
                i += 1;
            }
            // DSL 元组：递归进入（内容含类型表达式）
            TokenTree::Group(g) if g.delimiter() == delimiter![()] => {
                let inner: Vec<_> = g.stream().into_iter().collect();
                out.push(
                    Group::new(
                        g.delimiter(),
                        angle_collect(&inner)?.into_iter().collect(),
                    )
                    .into(),
                );
                i += 1;
            }
            // DSL 列表 / 宏体 / 属性：`ident![...]` 与 `#[...]` 透传（内容任意 Rust）
            TokenTree::Group(g) if g.delimiter() == delimiter![[]] => {
                if i > 0
                    && matches!(&tokens[i - 1], TokenTree::Punct(p)
                        if p.as_char() == '!' || p.as_char() == '#')
                {
                    out.push(tokens[i].clone());
                } else {
                    let inner: Vec<_> = g.stream().into_iter().collect();
                    out.push(
                        Group::new(
                            g.delimiter(),
                            angle_collect(&inner)?.into_iter().collect(),
                        )
                        .into(),
                    );
                }
                i += 1;
            }
            // 透传代码（body）：不进入，原样保留
            TokenTree::Group(_) => {
                out.push(tokens[i].clone());
                i += 1;
            }
            // 扁平 `<`：配对到匹配的 `>`（`->` 箭头的 `>` 不参与）
            TokenTree::Punct(p) if p.as_char() == '<' => {
                let Some(close) = find_angle_close(tokens, i) else {
                    return Err(compile_error_str(
                        "batch-impl: 未闭合的 `<`（缺少匹配的 `>`）",
                    ));
                };
                let inner: Vec<_> = tokens[i + 1..close].to_vec();
                out.push(
                    Group::new(
                        delimiter![<>],
                        angle_collect(&inner)?.into_iter().collect(),
                    )
                    .into(),
                );
                i = close + 1;
            }
            // 多余的 `>`（非箭头）：非法输入
            TokenTree::Punct(p) if p.as_char() == '>' && !is_arrow(tokens, i) => {
                return Err(compile_error_str(
                    "batch-impl: 多余的 `>`（缺少匹配的 `<`）",
                ));
            }
            _ => {
                out.push(tokens[i].clone());
                i += 1;
            }
        }
    }
    Ok(out)
}

/// 找 `tokens[open]`（`<`）的匹配 `>`：嵌套 `<` 深度跟踪，`->` 箭头的
/// `>` 不关闭。返回匹配 `>` 的索引；未闭合返回 `None`（`<` 保持扁平）。
fn find_angle_close(tokens: &[TokenTree], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (idx, token) in tokens.iter().enumerate().skip(open + 1) {
        if is_punct(token, '<') {
            depth += 1;
        } else if is_punct(token, '>') && !is_arrow(tokens, idx) {
            if depth == 0 {
                return Some(idx);
            }
            depth -= 1;
        }
    }
    None
}

fn is_punct(token: &TokenTree, ch: char) -> bool {
    matches!(token, TokenTree::Punct(p) if p.as_char() == ch)
}

/// 输出转换：递归把尖括号组（`delimiter![<>]`）还原为 `<` + 内容 + `>` 扁平 token。
/// 供三个宏入口的返回值收口（quote 插值会把尖括号组散布到输出各处）。
///
/// 递归规则与 [`angle_collect`] 一致：尖括号组 → 转 `<...>`（内部递归）；
/// `Paren`/`Bracket`（配对时递归进入过，内部可能有嵌套尖括号组）→ 重建并递归；
/// `Brace`（透传代码，`angle_collect` 从未进入 → 内部不可能有尖括号组）→
/// **原样透传，不重建**（保留 span，避免影响透传代码与诊断映射）。
pub(crate) fn render_angles(stream: TokenStream) -> TokenStream {
    let mut out = TokenStream::new();
    for tt in stream {
        match tt {
            TokenTree::Group(g) if g.delimiter() == delimiter![<>] => {
                let inner = render_angles(g.stream());
                out.extend([TokenTree::from(proc_macro2::Punct::new(
                    '<',
                    proc_macro2::Spacing::Alone,
                ))]);
                out.extend(inner);
                out.extend([TokenTree::from(proc_macro2::Punct::new(
                    '>',
                    proc_macro2::Spacing::Alone,
                ))]);
            }
            TokenTree::Group(g)
                if matches!(g.delimiter(), delimiter![()] | delimiter![[]]) =>
            {
                let inner = render_angles(g.stream());
                // 重建并恢复原 span（否则 doc 属性等 Bracket 组 span 变 call_site，
                // 影响 clippy 等基于 span 的诊断映射）
                let mut new_g = Group::new(g.delimiter(), inner);
                new_g.set_span(g.span());
                out.extend([TokenTree::Group(new_g)]);
            }
            // Brace（透传代码）：原样保留——内部不可能有尖括号组
            other => out.extend([other]),
        }
    }
    out
}
