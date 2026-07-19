use proc_macro2::{Span, TokenStream as TokenStream2, TokenTree};
use quote::quote;
use crate::core::types::{ImplSpec, SlotKind, ParseResult, err};
use crate::core::recursion::{RecursionGuard, span_suffix};
use crate::core::utils::*;
use crate::core::caret::expand_caret;

// ===========================================================================
// - 运算符（左结合元组构建）
// ===========================================================================

pub fn split_by_dash(tokens: &[TokenTree]) -> Option<(Vec<TokenTree>, Vec<TokenTree>)> {
    // 左结合：找最后一个 `-`，但排除 `->` 的情况
    let mut depth = 0u32;
    let mut last = None;
    for (i, tt) in tokens.iter().enumerate() {
        if is_punct(tt, '-') && depth == 0 {
            // 排除 `->` 的情况：`-` 后面紧跟 `>`
            let is_arrow = i + 1 < tokens.len() && is_punct(&tokens[i + 1], '>');
            if !is_arrow {
                last = Some(i);
            }
        }
        if is_punct(tt, '<') {
            depth += 1;
        } else if is_punct(tt, '>') {
            depth = depth.saturating_sub(1);
        }
    }
    last.map(|i| (tokens[..i].to_vec(), tokens[i + 1..].to_vec()))
}

/// 展开 `-` 运算符链（左结合）
/// `()-[A,B]-[C,D]` = `(()-[A,B])-[C,D]`
/// 每步：已有元组列表 × 新类型列表 → 新元组列表
pub fn expand_dash(
    tokens: TokenStream2,
    parent_types: &[TokenStream2],
    parent_trait: &Option<Vec<TokenStream2>>,
    body: Option<TokenStream2>,
    span: Span,
) -> ParseResult {
    let _guard = match RecursionGuard::new() {
        Ok(g) => g,
        Err(e) => return e,
    };
    let tv: Vec<TokenTree> = tokens.clone().into_iter().collect();
    if let Some((left, right)) = split_by_dash(&tv) {
        // 从左侧第一个 token 的 span 生成后缀
        let suffix = left.first().map(|t| span_suffix(t.span())).unwrap_or(0);
        // 左结合：递归处理左半部分
        let left_specs = match expand_dash(
            left.into_iter().collect(),
            parent_types,
            parent_trait,
            body.clone(),
            span,
        ) {
            ParseResult::Ok(s) => s,
            ParseResult::Err(e) => return ParseResult::Err(e),
        };

        // 解析右半部分为 slot 列表
        let right_ts: TokenStream2 = right.into_iter().collect();
        let right_slots = match dash_parse_slots(&right_ts, span) {
            Ok(s) => s,
            Err(e) => return e,
        };

        // 左结合：对每个已有结果 × 每个新 slot → 新结果
        let mut specs = Vec::new();
        for spec in &left_specs {
            for slot in &right_slots {
                let (new_target, extra_types) = dash_append(&spec.target, slot, suffix);
                let mut all_types = spec.type_params.clone();
                all_types.extend(extra_types);
                specs.push(ImplSpec {
                    type_params: all_types,
                    trait_params: spec.trait_params.clone(),
                    target: new_target,
                    custom_body: spec.custom_body.clone(),
                    is_unsafe: false,
                });
            }
        }
        ParseResult::Ok(specs)
    } else {
        // 没有 `-`：起始点
        // 可以是 `()`、`(A,)`、或单个类型
        dash_parse_start(tokens, parent_types, parent_trait, span)
    }
}

/// 解析 `-` 右侧的 slot 列表
/// `[A,B]` → slots; `T` → 单个 slot
/// 支持 `^` 展开：`[Box^u32, Vec^isize]` → `[Box<u32>, Vec<isize>]`
fn dash_parse_slots(ts: &TokenStream2, span: Span) -> Result<Vec<SlotKind>, ParseResult> {
    let tv: Vec<TokenTree> = ts.clone().into_iter().collect();
    if tv.is_empty() {
        return Err(ParseResult::Err(err(
            span,
            "- 右侧为空，期望类型或类型列表，如 -T 或 -[A,B]",
        )));
    }
    // 单个 bracket group → 按逗号分割
    if tv.len() == 1 {
        if let TokenTree::Group(ref g) = tv[0] {
            if g.delimiter() == proc_macro2::Delimiter::Bracket {
                let inner: TokenStream2 = g.stream();
                if has_top_level_char(&inner, ',') {
                    // 按逗号分割，每个 segment 都是 Fixed
                    let (segs, _) = split_raw(inner, ',');
                    return Ok(segs
                        .into_iter()
                        .map(|seg| {
                            let seg_ts: TokenStream2 = seg.into_iter().collect();
                            // 如果 segment 包含 ^，展开它
                            if has_top_level_char(&seg_ts, '^') {
                                // 使用 expand_caret 展开
                                let expanded = expand_caret(
                                    seg_ts.clone(),
                                    &[],
                                    &None,
                                    None,
                                    span,
                                );
                                match expanded {
                                    ParseResult::Ok(specs) if !specs.is_empty() => {
                                        // 取第一个结果的 target 作为 slot
                                        SlotKind::Fixed(specs[0].target.clone())
                                    }
                                    _ => SlotKind::Fixed(seg_ts),
                                }
                            } else {
                                SlotKind::Fixed(seg_ts)
                            }
                        })
                        .collect());
                }
                // 单个元素的 bracket → 检查是否包含 ^
                let inner_ts: TokenStream2 = g.stream();
                if has_top_level_char(&inner_ts, '^') {
                    let expanded = expand_caret(
                        inner_ts,
                        &[],
                        &None,
                        None,
                        span,
                    );
                    match expanded {
                        ParseResult::Ok(specs) if !specs.is_empty() => {
                            return Ok(vec![SlotKind::Fixed(specs[0].target.clone())]);
                        }
                        _ => {}
                    }
                }
                return Ok(vec![SlotKind::Fixed(ts.clone())]);
            }
            // 其他括号类型（元组等）→ 作为 Fixed
            return Ok(vec![SlotKind::Fixed(ts.clone())]);
        }
    }
    // 单个类型 → 检查是否包含 ^
    if has_top_level_char(ts, '^') {
        let expanded = expand_caret(
            ts.clone(),
            &[],
            &None,
            None,
            span,
        );
        match expanded {
            ParseResult::Ok(specs) if !specs.is_empty() => {
                return Ok(vec![SlotKind::Fixed(specs[0].target.clone())]);
            }
            _ => {}
        }
    }
    Ok(vec![SlotKind::Fixed(ts.clone())])
}

/// 解析 `-` 链的起始点（没有 `-` 时）
fn dash_parse_start(
    tokens: TokenStream2,
    parent_types: &[TokenStream2],
    parent_trait: &Option<Vec<TokenStream2>>,
    _span: Span,
) -> ParseResult {
    let tv: Vec<TokenTree> = tokens.clone().into_iter().collect();
    // `()` → 空元组起始
    if tv.len() == 1 {
        if let TokenTree::Group(ref g) = tv[0] {
            if g.delimiter() == proc_macro2::Delimiter::Parenthesis {
                let inner: Vec<TokenTree> = g.stream().into_iter().collect();
                if inner.is_empty() {
                    return ParseResult::Ok(vec![ImplSpec {
                        type_params: parent_types.to_vec(),
                        trait_params: parent_trait.clone(),
                        target: quote! { () },
                        custom_body: None,
                        is_unsafe: false,
                    }]);
                }
                // `(A,)` → 已有元组作为起始
                return ParseResult::Ok(vec![ImplSpec {
                    type_params: parent_types.to_vec(),
                    trait_params: parent_trait.clone(),
                    target: tokens,
                    custom_body: None,
                    is_unsafe: false,
                }]);
            }
        }
    }
    // 单个类型 → 作为 target
    ParseResult::Ok(vec![ImplSpec {
        type_params: parent_types.to_vec(),
        trait_params: parent_trait.clone(),
        target: tokens,
        custom_body: None,
        is_unsafe: false,
    }])
}

/// 将 slot 追加到已有元组末尾
/// `()` + `T` → `(T,)`  (generic_counter = 0 for bound)
/// `(A,)` + `T` → `(A, T)`
/// `(A,B)` + `T` → `(A, B, T)`
/// `A` + `T` → `A<T>` (泛型应用，当 A 不是元组时)
fn dash_append(tuple_ts: &TokenStream2, slot: &SlotKind, suffix: u64) -> (TokenStream2, Vec<TokenStream2>) {
    let tv: Vec<TokenTree> = tuple_ts.clone().into_iter().collect();
    let mut extra_types = Vec::new();

    // 检查是否是空元组 `()`
    let is_empty_tuple = tv.len() == 1
        && matches!(&tv[0], TokenTree::Group(g)
            if g.delimiter() == proc_macro2::Delimiter::Parenthesis && g.stream().into_iter().next().is_none());

    // 检查是否是元组 `(A,)` 或 `(A,B)`
    let is_tuple = tv.len() == 1
        && matches!(&tv[0], TokenTree::Group(g) if g.delimiter() == proc_macro2::Delimiter::Parenthesis);

    match slot {
        SlotKind::Fixed(fixed_ts) => {
            if is_empty_tuple {
                // () + T → (T,)
                let ts = quote! { ( #fixed_ts ,) };
                (ts, extra_types)
            } else if is_tuple {
                // (A,B,...) + T → (A,B,...,T)
                if let TokenTree::Group(g) = &tv[0] {
                    let content = g.stream();
                    // 去掉尾随逗号再追加
                    let trimmed = strip_trailing_comma(content);
                    let mut new_inner = trimmed;
                    new_inner.extend(std::iter::once(TokenTree::Punct(proc_macro2::Punct::new(
                        ',',
                        proc_macro2::Spacing::Alone,
                    ))));
                    new_inner.extend(fixed_ts.clone());
                    let new_group = proc_macro2::Group::new(g.delimiter(), new_inner);
                    let mut result = TokenStream2::new();
                    result.extend(std::iter::once(TokenTree::Group(new_group)));
                    (result, extra_types)
                } else {
                    (tuple_ts.clone(), extra_types)
                }
            } else {
                // A + T → A<T> (泛型应用)
                // 检查是否是带泛型的容器，如 HashMap<u32>
                // 如果是，追加参数：HashMap<u32> + String → HashMap<u32, String>
                if let Some(new_ts) = append_to_generic_container(tuple_ts, fixed_ts) {
                    (new_ts, extra_types)
                } else {
                    let ts = quote! { #tuple_ts < #fixed_ts > };
                    (ts, extra_types)
                }
            }
        }
        SlotKind::Bound(bound_ts) => {
            // 泛型参数
            let letter = crate::core::tuple::generic_letter(0, suffix);
            extra_types.push(quote! { #letter: #bound_ts });

            if is_empty_tuple {
                // () + <Bound> → (A: Bound,)
                let ts = quote! { ( #letter ,) };
                (ts, extra_types)
            } else if is_tuple {
                // (A,B,...) + <Bound> → (A,B,...,X,)  where X: Bound
                if let TokenTree::Group(g) = &tv[0] {
                    let content = g.stream();
                    let trimmed = strip_trailing_comma(content);
                    let mut new_inner = trimmed;
                    new_inner.extend(std::iter::once(TokenTree::Punct(proc_macro2::Punct::new(
                        ',',
                        proc_macro2::Spacing::Alone,
                    ))));
                    new_inner.extend(quote! { #letter });
                    let new_group = proc_macro2::Group::new(g.delimiter(), new_inner);
                    let mut result = TokenStream2::new();
                    result.extend(std::iter::once(TokenTree::Group(new_group)));
                    (result, extra_types)
                } else {
                    (tuple_ts.clone(), extra_types)
                }
            } else {
                // A + <Bound> → A<Letter>  where Letter: Bound
                // 检查是否是带泛型的容器
                let bound_letter: TokenStream2 = quote! { #letter };
                if let Some(new_ts) = append_to_generic_container(tuple_ts, &bound_letter) {
                    (new_ts, extra_types)
                } else {
                    let ts = quote! { #tuple_ts < #letter > };
                    (ts, extra_types)
                }
            }
        }
    }
}

/// 尝试向带泛型的容器追加参数
///
/// 检测 `Ident<...>` 模式，如果匹配则追加新参数而非嵌套泛型。
///
/// 示例：
/// - `HashMap<u32>` + `String` → `HashMap<u32, String>`
/// - `HashMap<u32>` + `Box<i32>` → `HashMap<u32, Box<i32>>`
///
/// 如果不是带泛型的容器（如普通 `Vec`），返回 None。
fn append_to_generic_container(container_ts: &TokenStream2, new_arg: &TokenStream2) -> Option<TokenStream2> {
    let tv: Vec<TokenTree> = container_ts.clone().into_iter().collect();
    
    // 检查是否是 Ident<...> 模式，如 HashMap<u32>
    if tv.len() >= 3 {
        if let (TokenTree::Ident(name), TokenTree::Punct(p)) = (&tv[0], &tv[1]) {
            if p.as_char() == '<' {
                // 找到最后的 >，提取中间的泛型参数
                let mut depth = 0u32;
                let mut last_gt_pos = None;
                for (i, tt) in tv.iter().enumerate().skip(2) {
                    if let TokenTree::Punct(pp) = tt {
                        if pp.as_char() == '<' {
                            depth += 1;
                        } else if pp.as_char() == '>' {
                            if depth == 0 {
                                last_gt_pos = Some(i);
                                break;
                            }
                            depth -= 1;
                        }
                    }
                }
                
                if let Some(gt_pos) = last_gt_pos {
                    // 检查 > 后面是否还有其他内容（如有，则不是简单的容器）
                    if gt_pos == tv.len() - 1 {
                        // 提取现有的泛型参数
                        let existing_args: Vec<&TokenTree> = tv[2..gt_pos].iter().collect();
                        
                        // 构建新的泛型参数列表：现有参数 + 新参数
                        let mut new_args = TokenStream2::new();
                        for (i, arg) in existing_args.iter().enumerate() {
                            if i > 0 {
                                new_args.extend(std::iter::once(TokenTree::Punct(
                                    proc_macro2::Punct::new(',', proc_macro2::Spacing::Alone),
                                )));
                            }
                            new_args.extend(std::iter::once((*arg).clone()));
                        }
                        new_args.extend(std::iter::once(TokenTree::Punct(
                            proc_macro2::Punct::new(',', proc_macro2::Spacing::Alone),
                        )));
                        new_args.extend(new_arg.clone());
                        
                        // 构建结果：Name<args>
                        let mut result = TokenStream2::new();
                        result.extend(std::iter::once(TokenTree::Ident(name.clone())));
                        result.extend(std::iter::once(TokenTree::Punct(
                            proc_macro2::Punct::new('<', proc_macro2::Spacing::Alone),
                        )));
                        result.extend(new_args);
                        result.extend(std::iter::once(TokenTree::Punct(
                            proc_macro2::Punct::new('>', proc_macro2::Spacing::Alone),
                        )));
                        
                        return Some(result);
                    }
                }
            }
        }
    }
    
    None
}
