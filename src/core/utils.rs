use proc_macro2::{Delimiter, TokenStream as TokenStream2, TokenTree};
use crate::core::types::err;

// ===========================================================================
// 工具函数
// ===========================================================================

/// 按字符分割 + <> 深度检查。ch: ',' 或 ';'
pub fn split_by(tokens: TokenStream2, ch: char, span: proc_macro2::Span) -> Result<Vec<TokenStream2>, TokenStream2> {
    let (segs, depth) = split_raw(tokens, ch);
    if depth > 0 {
        Err(err(span, &format!("未闭合的 `<`（{} 层）", depth)))
    } else {
        Ok(segs)
    }
}

/// 按字符分割（返回段列表 + 剩余 <> 深度）
pub fn split_raw(tokens: TokenStream2, ch: char) -> (Vec<TokenStream2>, u32) {
    let mut segs: Vec<Vec<TokenTree>> = Vec::new();
    let mut cur: Vec<TokenTree> = Vec::new();
    let mut depth = 0u32;
    for tt in tokens {
        if let TokenTree::Punct(p) = &tt {
            match p.as_char() {
                c if c == ch && depth == 0 => {
                    if !cur.is_empty() {
                        segs.push(std::mem::take(&mut cur));
                    }
                    continue;
                }
                '<' => depth += 1,
                '>' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
        cur.push(tt);
    }
    if !cur.is_empty() {
        segs.push(cur);
    }
    (
        segs.into_iter().map(|v| v.into_iter().collect()).collect(),
        depth,
    )
}

pub fn has_top_level_char(tokens: &TokenStream2, ch: char) -> bool {
    let mut depth = 0u32;
    for tt in tokens.clone() {
        if let TokenTree::Punct(ref p) = tt {
            match p.as_char() {
                c if c == ch && depth == 0 => return true,
                '<' => depth += 1,
                '>' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
    }
    false
}

/// 解析平衡 <> 内的逗号分隔项
pub fn parse_balanced(
    tokens: &[TokenTree],
    start: usize,
) -> (Result<Vec<TokenStream2>, String>, usize) {
    let mut args: Vec<Vec<TokenTree>> = vec![];
    let mut cur: Vec<TokenTree> = vec![];
    let mut depth = 0u32;
    let mut pos = start;
    while pos < tokens.len() {
        match &tokens[pos] {
            TokenTree::Punct(p) if p.as_char() == '>' => {
                if depth == 0 {
                    if !cur.is_empty() {
                        args.push(std::mem::take(&mut cur));
                    }
                    pos += 1;
                    return (
                        Ok(args.into_iter().map(|v| v.into_iter().collect()).collect()),
                        pos,
                    );
                }
                depth -= 1;
                cur.push(tokens[pos].clone());
            }
            TokenTree::Punct(p) if p.as_char() == '<' => {
                depth += 1;
                cur.push(tokens[pos].clone());
            }
            TokenTree::Punct(p) if p.as_char() == ',' && depth == 0 => {
                if !cur.is_empty() {
                    args.push(std::mem::take(&mut cur));
                }
            }
            _ => cur.push(tokens[pos].clone()),
        }
        pos += 1;
    }
    (Err("未闭合的 `<`".into()), pos)
}

pub fn split_trailing_brace(tokens: &[TokenTree]) -> (Option<Vec<TokenTree>>, Vec<TokenTree>) {
    if let Some(TokenTree::Group(g)) = tokens.last() {
        if g.delimiter() == Delimiter::Brace {
            return (
                Some(g.stream().into_iter().collect()),
                tokens[..tokens.len() - 1].to_vec(),
            );
        }
    }
    (None, tokens.to_vec())
}

pub fn find_top_level_colon(tokens: &[TokenTree]) -> Option<usize> {
    let mut depth = 0u32;
    let mut i = 0;
    while i < tokens.len() {
        if is_punct(&tokens[i], ':') && depth == 0 {
            if i + 1 < tokens.len() && is_punct(&tokens[i + 1], ':') {
                i += 2;
                continue;
            }
            return Some(i);
        }
        if is_punct(&tokens[i], '<') {
            depth += 1;
        } else if is_punct(&tokens[i], '>') {
            depth = depth.saturating_sub(1);
        }
        i += 1;
    }
    None
}

pub fn split_at_punct(tokens: &[TokenTree], ch: char) -> Option<(Vec<TokenTree>, Vec<TokenTree>)> {
    let mut depth = 0u32;
    for (i, tt) in tokens.iter().enumerate() {
        if is_punct(tt, ch) && depth == 0 {
            return Some((tokens[..i].to_vec(), tokens[i + 1..].to_vec()));
        }
        if is_punct(tt, '<') {
            depth += 1;
        } else if is_punct(tt, '>') {
            depth = depth.saturating_sub(1);
        }
    }
    None
}

/// 去掉 TokenStream 末尾的逗号
pub fn strip_trailing_comma(ts: TokenStream2) -> TokenStream2 {
    let mut tokens: Vec<TokenTree> = ts.into_iter().collect();
    if matches!(tokens.last(), Some(TokenTree::Punct(p)) if p.as_char() == ',') {
        tokens.pop();
    }
    tokens.into_iter().collect()
}

pub fn is_punct(tt: &TokenTree, ch: char) -> bool {
    matches!(tt, TokenTree::Punct(p) if p.as_char() == ch)
}

pub fn is_ident_eq(tt: &TokenTree, id: &proc_macro2::Ident) -> bool {
    matches!(tt, TokenTree::Ident(i) if i == id)
}

pub fn tokens_span(tokens: &TokenStream2) -> proc_macro2::Span {
    tokens
        .clone()
        .into_iter()
        .next()
        .map(|t| t.span())
        .unwrap_or_else(proc_macro2::Span::call_site)
}
