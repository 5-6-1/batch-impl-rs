use proc_macro2::{Ident, Spacing, TokenTree};

/// 探测 attr 起首的 `# Path :` 路径前缀。
///
/// 规则：以 `#` + `Ident` + (`::` `Ident`)+ + `:` 形式起始时，
/// 返回 `Some((path_tokens, last_ident, 余下 tokens))`，其中
/// `path_tokens` 不含首 `#` 和尾 `:`；否则返回 `None`，
/// 调用方按原 DSL 处理整 attr。
///
/// **要求至少一个 `::`**：`#Display: ...` 不匹配（单段 trait 名
/// 通过 dummy trait ident 隐式给出，无需路径前缀）。这避免了
/// `#Display: ...` 被误识别为路径前缀而把 `Display` 吞掉的歧义。
pub(crate) fn try_parse_path_prefix(
    tokens: &[TokenTree],
) -> Option<(Vec<TokenTree>, Option<Ident>, Vec<TokenTree>)> {
    // 形式：# Ident (:: Ident)+ :   token 数最少 5: # Ident :: Ident :
    if tokens.len() < 5 {
        return None;
    }
    if !matches!(&tokens[0], TokenTree::Punct(p) if p.as_char() == '#') {
        return None;
    }
    if !matches!(&tokens[1], TokenTree::Ident(_)) {
        return None;
    }
    // 状态机：
    //   expect_sep=true  → 期待 `::`（双 `:` Punct，第二 `:` 可 Alone）
    //   expect_sep=false → 期待 Ident（紧跟在 `::` 之后）
    // 起始：刚读完第一个 Ident，期待 `::`。
    let mut i = 2usize;
    let mut expect_sep = true;
    let mut saw_double_colon = false;
    let mut last_ident = None;
    loop {
        match tokens.get(i) {
            // `::`：必须是连续两个 `:`，且第一个 `:` Spacing::Joint
            Some(TokenTree::Punct(p)) if p.as_char() == ':' && expect_sep => {
                match tokens.get(i + 1) {
                    Some(TokenTree::Punct(p2))
                        if p2.as_char() == ':' && p.spacing() == Spacing::Joint =>
                    {
                        i += 2;
                        expect_sep = false;
                        saw_double_colon = true;
                    }
                    // 单 `:` 收尾——只在已见过至少一个 `::` 时才接受
                    _ if saw_double_colon => {
                        let path = tokens[1..i].to_vec();
                        let rest = tokens[i + 1..].to_vec();
                        return Some((path, last_ident, rest));
                    }
                    // 否则 falls through 到下方 `_ => return None`
                    _ => return None,
                }
            }
            // 期待的 Ident（紧跟在 `::` 之后）
            Some(TokenTree::Ident(id)) if !expect_sep => {
                i += 1;
                last_ident = Some(id.clone());
                expect_sep = true;
            }
            _ => return None,
        }
    }
}
