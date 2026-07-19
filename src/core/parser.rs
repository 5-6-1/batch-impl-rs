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
        match parse_segment(seg, &[], &None, &[], trait_name) {
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

/// 将 trait 泛型参数分离为普通参数和关联类型绑定
/// `Item=T` → 关联类型绑定 (Item, T)
/// `T` → 普通参数
fn split_trait_params(
    params: Vec<TokenStream2>,
) -> (Vec<TokenStream2>, Vec<(TokenStream2, TokenStream2)>) {
    let mut regular = Vec::new();
    let mut assoc = Vec::new();
    for p in params {
        let tokens: Vec<TokenTree> = p.clone().into_iter().collect();
        // 查找顶层 `=`（排除 `==` 和 `=>`）
        let mut eq_pos = None;
        for (i, tt) in tokens.iter().enumerate() {
            if is_punct(tt, '=') {
                // 排除 ==
                if i + 1 < tokens.len() && is_punct(&tokens[i + 1], '=') {
                    continue;
                }
                // 排除 =>
                if i + 1 < tokens.len() && is_punct(&tokens[i + 1], '>') {
                    continue;
                }
                eq_pos = Some(i);
                break;
            }
        }
        if let Some(pos) = eq_pos {
            let name: TokenStream2 = tokens[..pos].iter().cloned().collect();
            let value: TokenStream2 = tokens[pos + 1..].iter().cloned().collect();
            if name.is_empty() || value.is_empty() {
                // 格式错误，当作普通参数
                regular.push(p);
            } else {
                assoc.push((name, value));
            }
        } else {
            regular.push(p);
        }
    }
    (regular, assoc)
}

pub fn parse_segment(
    tokens: TokenStream2,
    parent_types: &[TokenStream2],
    parent_trait: &Option<Vec<TokenStream2>>,
    parent_assoc: &[(TokenStream2, TokenStream2)],
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

    // 2. trait 泛型（可能包含关联类型绑定 Item=T）
    let (own_trait_raw, trait_consumed) = if pos < total
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

    // 分离普通参数和关联类型绑定
    let (own_trait, own_assoc) = match own_trait_raw {
        Some(params) => split_trait_params(params),
        None => (vec![], vec![]),
    };

    let eff_trait = if own_trait.is_empty() {
        parent_trait.clone()
    } else {
        let mut m = parent_trait.clone().unwrap_or_default();
        m.extend(own_trait);
        Some(m)
    };

    // 合并关联类型绑定
    let mut eff_assoc = parent_assoc.to_vec();
    eff_assoc.extend(own_assoc);

    // 3. 分离 {body}, 4. ^ 运算符（高优先级）, 4b. - 运算符（低优先级）, 5. 有效性检查, 6. 目标类型
    let remaining = &tv[pos..];
    let (body_vec, target_vec) = split_trailing_brace(remaining);
    let body: Option<TokenStream2> = body_vec.map(|v| v.into_iter().collect());

    // 特殊处理 fn- 前缀：fn-(A,B)^N 生成多个函数类型
    if target_vec.len() >= 2
        && matches!(&target_vec[0], TokenTree::Ident(id) if id == "fn")
        && is_punct(&target_vec[1], '-')
    {
        // fn-... 格式：跳过 fn 和 -，解析剩余部分
        let dash_input: TokenStream2 = target_vec[2..].iter().cloned().collect();
        let dash_input_tv: Vec<TokenTree> = dash_input.clone().into_iter().collect();

        // 检查是否包含 ^，如果有，先展开生成所有组合
        if has_top_level_char(&dash_input, '^') {
            // 使用 expand_caret 生成所有组合
            match expand_caret(dash_input.clone(), &eff_types, &eff_trait, &eff_assoc, body.clone(), span) {
                ParseResult::Ok(combo_specs) => {
                    // 每个组合包装成 fn(...)
                    let mut specs = Vec::new();
                    for combo in combo_specs {
                        let fn_target = {
                            let target_tv: Vec<TokenTree> = combo.target.clone().into_iter().collect();
                            let mut result = TokenStream2::new();
                            result.extend(std::iter::once(TokenTree::Ident(proc_macro2::Ident::new("fn", proc_macro2::Span::call_site()))));
                            // 如果目标已经是元组 (A, B)，直接用作参数列表
                            if target_tv.len() == 1 && matches!(&target_tv[0], TokenTree::Group(g) if g.delimiter() == proc_macro2::Delimiter::Parenthesis) {
                                result.extend(combo.target.clone());
                            } else {
                                // 否则包装成 (target)
                                let inner = proc_macro2::Group::new(proc_macro2::Delimiter::Parenthesis, combo.target);
                                result.extend(std::iter::once(TokenTree::Group(inner)));
                            }
                            result
                        };
                        specs.push(ImplSpec {
                            type_params: combo.type_params,
                            trait_params: combo.trait_params,
                            assoc_bindings: combo.assoc_bindings,
                            target: fn_target,
                            custom_body: combo.custom_body,
                            is_unsafe: false,
                    attributes: vec![],
                        });
                    }
                    return ParseResult::Ok(specs);
                }
                ParseResult::Err(e) => return ParseResult::Err(e),
            }
        }

        // 没有 ^，使用 dash 追加参数
        let fn_ts: TokenStream2 = std::iter::once(target_vec[0].clone()).collect();
        let start_specs = vec![ImplSpec {
            type_params: eff_types.to_vec(),
            trait_params: eff_trait.clone(),
            assoc_bindings: eff_assoc.to_vec(),
            target: fn_ts,
            custom_body: body.clone(),
            is_unsafe: false,
                    attributes: vec![],
        }];
        let right_slots = match crate::core::dash::dash_parse_slots_public(&dash_input, span) {
            Ok(s) => s,
            Err(e) => return e,
        };
        let mut specs = Vec::new();
        for spec in &start_specs {
            for slot in &right_slots {
                let (new_target, extra_types) = crate::core::dash::dash_append_public(&spec.target, slot, 0);
                let mut all_types = spec.type_params.clone();
                all_types.extend(extra_types);
                specs.push(ImplSpec {
                    type_params: all_types,
                    trait_params: spec.trait_params.clone(),
                    assoc_bindings: spec.assoc_bindings.clone(),
                    target: new_target,
                    custom_body: spec.custom_body.clone(),
                    is_unsafe: false,
                    attributes: vec![],
                });
            }
        }
        return ParseResult::Ok(specs);
    }

    // ^ 优先级高于 -：先检查 ^
    if split_by_caret(&target_vec).is_some() {
        return expand_caret(
            target_vec.into_iter().collect(),
            &eff_types,
            &eff_trait,
            &eff_assoc,
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
            &eff_assoc,
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

    parse_target(target_ts, &eff_types, &eff_trait, &eff_assoc, trait_name, body)
}

fn parse_target(
    tokens: TokenStream2,
    parent_types: &[TokenStream2],
    parent_trait: &Option<Vec<TokenStream2>>,
    parent_assoc: &[(TokenStream2, TokenStream2)],
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
        assoc_bindings: parent_assoc.to_vec(),
        target: ts,
        custom_body: b,
        is_unsafe: false,
                    attributes: vec![],
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
                    match parse_segment(sub, parent_types, parent_trait, parent_assoc, trait_name) {
                        ParseResult::Ok(ss) => {
                            for mut s in ss {
                                // 合并独立 body 和共享 body
                                match (&s.custom_body, &bb) {
                                    (Some(_independent), Some(shared)) => {
                                        // 独立 body + 共享 body → 拼接（独立在后，覆盖同名方法由编译器报错）
                                        let mut merged = shared.clone();
                                        merged.extend(s.custom_body.take().unwrap());
                                        s.custom_body = Some(merged);
                                    }
                                    (None, Some(shared)) => {
                                        s.custom_body = Some(shared.clone());
                                    }
                                    _ => {}
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
