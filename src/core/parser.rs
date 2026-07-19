use proc_macro2::{Delimiter, Span, TokenStream as TokenStream2, TokenTree};
use syn::Ident;
use crate::core::types::{ImplSpec, ParseResult, err};
use crate::core::utils::*;
use crate::core::caret::{split_by_caret, expand_caret};
use crate::core::dash::{split_by_dash, expand_dash};

// ===========================================================================
// 解析器
// ===========================================================================

pub fn parse_top_level(tokens: TokenStream2, trait_name: &Ident, trait_span: Span) -> ParseResult {
    if tokens.is_empty() {
        return ParseResult::Err(err(
            trait_span,
            "batch_impl 至少需要一个类型参数，如 #[batch_impl(usize)]",
        ));
    }
    let attr_span = tokens_span(&tokens);
    let segments = match split_by(tokens, ',', attr_span) {
        Ok(s) => s,
        Err(e) => return ParseResult::Err(e),
    };
    let mut all = Vec::new();
    for seg in segments {
        match parse_segment(seg, &[], &None, trait_name) {
            ParseResult::Ok(s) => all.extend(s),
            ParseResult::Err(e) => return ParseResult::Err(e),
        }
    }
    if all.is_empty() {
        return ParseResult::Err(err(
            trait_span,
            "batch_impl 解析后没有生成任何 impl，请检查语法。示例: #[batch_impl(usize, isize)]",
        ));
    }
    ParseResult::Ok(all)
}

pub fn parse_segment(
    tokens: TokenStream2,
    parent_types: &[TokenStream2],
    parent_trait: &Option<Vec<TokenStream2>>,
    trait_name: &Ident,
) -> ParseResult {
    let span = tokens_span(&tokens);
    let tv: Vec<TokenTree> = tokens.into_iter().collect();
    if tv.is_empty() {
        return ParseResult::Err(err(span, "空的实现规格，期望类型或表达式"));
    }

    let mut pos = 0;
    let total = tv.len();

    // 1. impl 泛型
    let own_types = if pos < total && is_punct(&tv[pos], '<') {
        pos += 1;
        let (r, np) = parse_balanced(&tv, pos);
        pos = np;
        match r {
            Ok(v) => v,
            Err(m) => return ParseResult::Err(err(span, &m)),
        }
    } else {
        vec![]
    };

    let eff_types = {
        let mut m = parent_types.to_vec();
        for p in own_types {
            let s = p.to_string();
            if !m.iter().any(|x| x.to_string() == s) {
                m.push(p);
            }
        }
        m
    };

    // 2. trait 泛型
    let (own_trait, trait_consumed) = if pos < total
        && is_ident_eq(&tv[pos], trait_name)
        && pos + 1 < total
        && is_punct(&tv[pos + 1], '<')
    {
        pos += 2;
        let (r, np) = parse_balanced(&tv, pos);
        pos = np;
        (
            Some(match r {
                Ok(v) => v,
                Err(m) => return ParseResult::Err(err(span, &m)),
            }),
            true,
        )
    } else {
        (None, false)
    };

    let eff_trait = own_trait.or_else(|| parent_trait.clone());

    // 3. 分离 {body}, 4. ^ 运算符（高优先级）, 4b. - 运算符（低优先级）, 5. 有效性检查, 6. 目标类型
    let remaining = &tv[pos..];
    let (body_vec, target_vec) = split_trailing_brace(remaining);
    let body: Option<TokenStream2> = body_vec.map(|v| v.into_iter().collect());

    // ^ 优先级高于 -：先检查 ^
    if split_by_caret(&target_vec).is_some() {
        return expand_caret(
            target_vec.into_iter().collect(),
            &eff_types,
            &eff_trait,
            body,
            span,
        );
    }

    // - 左结合元组构建（低优先级）
    if split_by_dash(&target_vec).is_some() {
        return expand_dash(
            target_vec.into_iter().collect(),
            &eff_types,
            &eff_trait,
            body,
            span,
        );
    }

    let target_ts: TokenStream2 = target_vec.into_iter().collect();
    if target_ts.is_empty() {
        if trait_consumed {
            return ParseResult::Err(err(span, &format!(
                "`{}<...>` 被解析为 trait 泛型参数，但缺少目标类型。若 `{}` 本身是目标类型，请去掉尖括号",
                trait_name, trait_name
            )));
        }
        return ParseResult::Err(err(span, "缺少目标类型，如 #[batch_impl(usize)]"));
    }

    parse_target(target_ts, &eff_types, &eff_trait, trait_name, body)
}

fn parse_target(
    tokens: TokenStream2,
    parent_types: &[TokenStream2],
    parent_trait: &Option<Vec<TokenStream2>>,
    trait_name: &Ident,
    inherited_body: Option<TokenStream2>,
) -> ParseResult {
    let span = tokens_span(&tokens);
    let tv: Vec<TokenTree> = tokens.into_iter().collect();
    if tv.is_empty() {
        return ParseResult::Err(err(span, "缺少目标类型，如 #[batch_impl(usize)]"));
    }

    let spec = |ts, b| ImplSpec {
        type_params: parent_types.to_vec(),
        trait_params: parent_trait.clone(),
        target: ts,
        custom_body: b,
        is_unsafe: false,
    };

    if let TokenTree::Group(ref g) = tv[0] {
        if g.delimiter() == Delimiter::Bracket {
            let inner = g.stream();
            let (b_body, extra) = split_trailing_brace(&tv[1..]);
            let bb: Option<TokenStream2> =
                b_body.map(|v| v.into_iter().collect()).or(inherited_body);

            if !extra.is_empty() {
                let mut ts = TokenStream2::new();
                ts.extend(std::iter::once(TokenTree::Group(g.clone())));
                ts.extend(extra);
                return ParseResult::Ok(vec![spec(ts, bb)]);
            }

            if has_top_level_char(&inner, ',') {
                let subs = match split_by(inner, ',', span) {
                    Ok(s) => s,
                    Err(e) => return ParseResult::Err(e),
                };
                if subs.is_empty() {
                    return ParseResult::Err(err(span, "空的并列列表"));
                }
                let mut results = Vec::new();
                for sub in subs {
                    match parse_segment(sub, parent_types, parent_trait, trait_name) {
                        ParseResult::Ok(ss) => {
                            for mut s in ss {
                                if s.custom_body.is_none() {
                                    s.custom_body = bb.clone();
                                }
                                results.push(s);
                            }
                        }
                        ParseResult::Err(e) => return ParseResult::Err(e),
                    }
                }
                return ParseResult::Ok(results);
            }

            if inner.is_empty() {
                return ParseResult::Err(err(
                    span,
                    "空的 `[]`——并列列表请填类型(如 [A,B])，切片请写 `[T]`",
                ));
            }

            let mut ts = TokenStream2::new();
            ts.extend(std::iter::once(TokenTree::Group(g.clone())));
            return ParseResult::Ok(vec![spec(ts, bb)]);
        }
    }

    ParseResult::Ok(vec![spec(tv.into_iter().collect(), inherited_body)])
}
