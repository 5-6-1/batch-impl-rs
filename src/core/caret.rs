use crate::core::recursion::{span_suffix, RecursionGuard};
use crate::core::tuple::*;
use crate::core::types::{display_tokens, err, ImplSpec, ParseResult, PrefixItem, TargetItem};
use crate::core::utils::*;
use proc_macro2::{Delimiter, Span, TokenStream as TokenStream2, TokenTree};
use quote::quote;

// ===========================================================================
// ^ 运算符相关函数
// ===========================================================================

pub fn split_by_caret(tokens: &[TokenTree]) -> Option<(Vec<TokenTree>, Vec<TokenTree>)> {
    split_at_punct(tokens, '^')
}

pub fn parse_prefix_items(tokens: &[TokenTree]) -> Result<Vec<PrefixItem>, String> {
    if tokens.is_empty() {
        return Err("^ 左侧缺少前缀，期望 &, self, Box 等".into());
    }
    if tokens.len() == 1 {
        if let TokenTree::Group(ref g) = tokens[0] {
            if g.delimiter() == Delimiter::Bracket {
                let inner: Vec<TokenTree> = g.stream().into_iter().collect();
                if inner.is_empty() {
                    return Err("^ 左侧 [] 为空，期望前缀列表，如 [&,Box]".into());
                }
                let (segs, d) = split_raw(inner.into_iter().collect(), ',');
                if d > 0 {
                    return Err("^ 左侧 [] 内尖括号不匹配，检查 <> 是否配对".into());
                }
                return segs
                    .iter()
                    .map(|s| parse_single_prefix(&s.clone().into_iter().collect::<Vec<_>>()))
                    .collect();
            }
        }
    }
    Ok(vec![parse_single_prefix(tokens)?])
}

pub fn parse_single_prefix(tokens: &[TokenTree]) -> Result<PrefixItem, String> {
    // 元组生成：() 或 (<bound>) 或 (A,) 或 (A,B)
    // 注意：(A) 无逗号 = 分组，不是元组
    if let [TokenTree::Group(ref g)] = tokens {
        if g.delimiter() == Delimiter::Parenthesis {
            let inner: Vec<TokenTree> = g.stream().into_iter().collect();
            if inner.is_empty() {
                // () = 空元组
                return Ok(PrefixItem::Tuple {
                    elem: None,
                    bound: None,
                });
            }
            if inner.len() >= 2 && matches!(&inner[0], TokenTree::Punct(p) if p.as_char() == '<') {
                // (<bound>)
                let (result, _) = parse_balanced(&inner, 1);
                let bound_tokens = match result {
                    Ok(args) if args.len() == 1 => args.into_iter().next().unwrap(),
                    _ => return Err("元组 bound 格式错误，期望 (<Trait>) 如 (<Clone>)".to_string()),
                };
                return Ok(PrefixItem::Tuple {
                    elem: None,
                    bound: Some(bound_tokens),
                });
            }
            // 检查是否有顶层逗号：有逗号 = 元组 (A,) 或 (A,B)，无逗号 = 分组 (A)
            let inner_ts: TokenStream2 = inner.into_iter().collect();
            if has_top_level_char(&inner_ts, ',') {
                // 去掉尾随逗号：(A,) → elem = A，而非 A,
                let trimmed = strip_trailing_comma(inner_ts);
                return Ok(PrefixItem::Tuple {
                    elem: Some(trimmed),
                    bound: None,
                });
            }
            // (A) 无逗号 → 分组，不作为元组前缀，回退到普通匹配
        }
    }
    match tokens {
        [TokenTree::Ident(id)] if id == "self" => Ok(PrefixItem::Self_),
        [TokenTree::Punct(p)] if p.as_char() == '&' => Ok(PrefixItem::Ref),
        [TokenTree::Punct(p), TokenTree::Ident(id)] if p.as_char() == '&' && id == "mut" => {
            Ok(PrefixItem::RefMut)
        }
        // *const 和 *mut
        [TokenTree::Punct(p), TokenTree::Ident(id)] if p.as_char() == '*' && id == "const" => {
            Ok(PrefixItem::ConstPtr)
        }
        [TokenTree::Punct(p), TokenTree::Ident(id)] if p.as_char() == '*' && id == "mut" => {
            Ok(PrefixItem::MutPtr)
        }
        [TokenTree::Ident(id)] if id == "unsafe" => Ok(PrefixItem::Unsafe),
        [TokenTree::Ident(id)] => Ok(PrefixItem::Container {
            name: id.clone(),
            prefill: None,
        }),
        // 容器带预填泛型：Ident<...>，如 HashMap<K>
        [TokenTree::Ident(id), TokenTree::Punct(p), ..] if p.as_char() == '<' => {
            let (result, _) = parse_balanced(tokens, 2);
            let prefill = match result {
                Ok(args) if !args.is_empty() => Some(args),
                _ => return Err(format!("容器 `{}` 的泛型参数解析失败", id)),
            };
            Ok(PrefixItem::Container {
                name: id.clone(),
                prefill,
            })
        }
        // 检测 (T) 无逗号的情况，提示用户加逗号
        [TokenTree::Group(g)] if g.delimiter() == Delimiter::Parenthesis => {
            let inner: Vec<TokenTree> = g.stream().into_iter().collect();
            if inner.len() == 1 && matches!(&inner[0], TokenTree::Ident(_)) {
                Err(format!(
                    "`({})` 是分组而非元组，若需单元素元组请写 `({},)`",
                    display_tokens(&inner),
                    display_tokens(&inner)
                ))
            } else {
                Err(format!(
                    "无法识别的 ^ 前缀: `{}`。支持: self, &, &mut, unsafe, 标识符(如Box), (), (<bound>)",
                    display_tokens(tokens)
                ))
            }
        }
        _ => Err(format!(
            "无法识别的 ^ 前缀: `{}`。支持: self, &, &mut, unsafe, 标识符(如Box), (), (<bound>)",
            display_tokens(tokens)
        )),
    }
}

pub fn parse_target_items(tokens: &[TokenTree]) -> Result<Vec<TargetItem>, String> {
    if tokens.is_empty() {
        return Err("^ 右侧缺少目标".into());
    }
    if tokens.len() == 1 {
        if let TokenTree::Group(ref g) = tokens[0] {
            if g.delimiter() == Delimiter::Bracket {
                let inner: Vec<TokenTree> = g.stream().into_iter().collect();
                if inner.is_empty() {
                    return Err("^ 右侧 [] 为空，期望类型列表，如 ^[A,B]".into());
                }
                let (segs, d) = split_raw(inner.into_iter().collect(), ',');
                if d > 0 {
                    return Err("^ 右侧 [] 内尖括号不匹配".into());
                }
                if segs.len() == 1 {
                    return Ok(vec![TargetItem::Single(tokens.iter().cloned().collect())]);
                }
                return Ok(segs.into_iter().map(TargetItem::Single).collect());
            }
        }
    }
    if tokens.len() >= 2 && is_punct(&tokens[0], '<') {
        let (r, _) = parse_balanced(tokens, 1);
        return r.map(|a| vec![TargetItem::Multi(a)]);
    }
    Ok(vec![TargetItem::Single(tokens.iter().cloned().collect())])
}

/// 应用 `^` 运算符，生成类型
///
/// 支持的模式：
/// - `self^T` → `T`
/// - `&^T` → `&T`
/// - `&mut^T` → `&mut T`
/// - `*const^T` → `*const T`
/// - `*mut^T` → `*mut T`
/// - `unsafe^T` → `T`（标记为 unsafe impl）
/// - `A^B` → `A<B>`
/// - `A^<X,Y>` → `A<X, Y>`
/// - `A<B>^C` → `A<B, C>`（预填泛型追加）
/// - `A<B>^<X,Y>` → `A<B, X, Y>`（预填泛型追加）
/// - `&^A^B` → `&A<B>`（引用类修饰符链式应用）
pub fn apply_caret(prefix: &PrefixItem, target: &TargetItem) -> Result<TokenStream2, String> {
    match (prefix, target) {
        (PrefixItem::Self_, TargetItem::Single(ts)) => Ok(ts.clone()),
        (PrefixItem::Self_, _) => Err("self^ 不能用于多参目标，如 <X,Y>".into()),
        (PrefixItem::Ref, TargetItem::Single(ts)) => Ok(quote! { & #ts }),
        (PrefixItem::Ref, _) => Err("&^ 不能用于多参目标，如 <X,Y>".into()),
        (PrefixItem::RefMut, TargetItem::Single(ts)) => Ok(quote! { &mut #ts }),
        (PrefixItem::RefMut, _) => Err("&mut^ 不能用于多参目标，如 <X,Y>".into()),
        (PrefixItem::ConstPtr, TargetItem::Single(ts)) => Ok(quote! { *const #ts }),
        (PrefixItem::ConstPtr, _) => Err("*const^ 不能用于多参目标，如 <X,Y>".into()),
        (PrefixItem::MutPtr, TargetItem::Single(ts)) => Ok(quote! { *mut #ts }),
        (PrefixItem::MutPtr, _) => Err("*mut^ 不能用于多参目标，如 <X,Y>".into()),
        (PrefixItem::Unsafe, TargetItem::Single(ts)) => Ok(ts.clone()),
        (PrefixItem::Unsafe, _) => Err("unsafe^ 不能用于多参目标".into()),
        (PrefixItem::Container { name, prefill }, TargetItem::Single(ts)) => {
            match prefill {
                Some(args) => {
                    // 有预填泛型：追加参数 A<B>^C → A<B, C>
                    Ok(quote! { #name < #(#args),* , #ts > })
                }
                None => {
                    // 无预填泛型：正常包装 A^B → A<B>
                    Ok(quote! { #name < #ts > })
                }
            }
        }
        (PrefixItem::Container { name, prefill }, TargetItem::Multi(args)) => {
            match prefill {
                Some(prefill_args) => {
                    // 有预填泛型：追加参数 A<B>^<X,Y> → A<B, X, Y>
                    Ok(quote! { #name < #(#prefill_args),* , #(#args),* > })
                }
                None => {
                    // 无预填泛型：正常包装 A^<X,Y> → A<X, Y>
                    Ok(quote! { #name < #(#args),* > })
                }
            }
        }
        (PrefixItem::Tuple { .. }, _) => Err("元组 ^ 的内部错误，这不应该发生，请报告 bug".into()),
    }
}

pub fn expand_caret(
    tokens: TokenStream2,
    types: &[TokenStream2],
    tr: &Option<Vec<TokenStream2>>,
    assoc: &[(TokenStream2, TokenStream2)],
    body: Option<TokenStream2>,
    span: Span,
) -> ParseResult {
    let _guard = match RecursionGuard::new() {
        Ok(g) => g,
        Err(e) => return e,
    };
    let tv: Vec<TokenTree> = tokens.clone().into_iter().collect();
    let (left, right) = split_by_caret(&tv).unwrap_or_else(|| (vec![], vec![]));

    // 从左侧第一个 token 的 span 生成后缀
    // 这样同一个 (...)^... 中的所有泛型参数共享相同后缀
    let suffix = left.first().map(|t| span_suffix(t.span())).unwrap_or(0);

    let prefixes = match parse_prefix_items(&left) {
        Ok(p) => p,
        Err(e) => return ParseResult::Err(err(span, &e)),
    };

    // 检测 unsafe 前缀并从列表中移除
    let has_unsafe = prefixes.iter().any(|p| matches!(p, PrefixItem::Unsafe));
    let prefixes: Vec<_> = prefixes
        .into_iter()
        .filter(|p| !matches!(p, PrefixItem::Unsafe))
        .collect();
    let targets = match parse_target_items(&right) {
        Ok(t) => t,
        Err(e) => return ParseResult::Err(err(span, &e)),
    };
    let targets = match expand_targets_recursive(targets, span) {
        Ok(t) => t,
        Err(e) => return e,
    };

    let mut specs = Vec::new();
    let mut simple_targets: Vec<TargetItem> = Vec::new();

    // 展开 bracket list 并递归处理含 ^ 的 target
    for t in &targets {
        if let TargetItem::Single(ts) = t {
            let tv: Vec<TokenTree> = ts.clone().into_iter().collect();
            // bracket group with comma → split and process each
            if tv.len() == 1 {
                if let TokenTree::Group(ref g) = tv[0] {
                    if g.delimiter() == Delimiter::Bracket {
                        let inner_ts: TokenStream2 = g.stream();
                        if has_top_level_char(&inner_ts, ',') {
                            let (segs, _) = split_raw(inner_ts, ',');
                            for seg in segs {
                                let stv: Vec<TokenTree> = seg.into_iter().collect();
                                if split_by_caret(&stv).is_some() {
                                    match expand_caret(
                                        stv.into_iter().collect(),
                                        types,
                                        tr,
                                        assoc,
                                        body.clone(),
                                        span,
                                    ) {
                                        ParseResult::Ok(sub) => specs.extend(sub),
                                        ParseResult::Err(e) => return ParseResult::Err(e),
                                    }
                                } else {
                                    simple_targets
                                        .push(TargetItem::Single(stv.into_iter().collect()));
                                }
                            }
                            continue;
                        }
                        // bracket 内无逗号：可能是 expand_single 退化后的结果
                        // 递归解包 bracket 层级，找到最内层内容后按逗号分割
                        let mut unwrap_count = 0usize;
                        let mut probe_ts = inner_ts.clone();
                        loop {
                            let pvec: Vec<TokenTree> = probe_ts.clone().into_iter().collect();
                            if pvec.len() == 1 {
                                if let TokenTree::Group(ref pg) = pvec[0] {
                                    if pg.delimiter() == Delimiter::Bracket {
                                        let pinner: TokenStream2 = pg.stream();
                                        if !has_top_level_char(&pinner, ',') {
                                            unwrap_count += 1;
                                            probe_ts = pinner;
                                            continue;
                                        }
                                    }
                                }
                            }
                            break;
                        }
                        // 无论是否含 ^，都按逗号分割并逐个处理
                        let (inner_segs, _) = split_raw(probe_ts, ',');
                        let mut any_expanded = false;
                        for seg in inner_segs {
                            let stv: Vec<TokenTree> = seg.into_iter().collect();
                            if split_by_caret(&stv).is_some() {
                                any_expanded = true;
                                match expand_caret(
                                    stv.into_iter().collect(),
                                    types,
                                    tr,
                                    assoc,
                                    body.clone(),
                                    span,
                                ) {
                                    ParseResult::Ok(sub) => {
                                        for spec in sub {
                                            let mut target = spec.target;
                                            for _ in 0..unwrap_count {
                                                let mut wrapped = TokenStream2::new();
                                                wrapped.extend(std::iter::once(TokenTree::Group(
                                                    proc_macro2::Group::new(
                                                        Delimiter::Bracket,
                                                        target,
                                                    ),
                                                )));
                                                target = wrapped;
                                            }
                                            // 放入 simple_targets 让外层前缀包装
                                            simple_targets.push(TargetItem::Single(target));
                                        }
                                    }
                                    ParseResult::Err(e) => return ParseResult::Err(e),
                                }
                            } else if unwrap_count > 0 {
                                // 非 ^ segment 但来自嵌套 bracket → 包装 unwrap_count-1 层
                                any_expanded = true;
                                let mut target: TokenStream2 = stv.into_iter().collect();
                                for _ in 0..unwrap_count - 1 {
                                    let mut wrapped = TokenStream2::new();
                                    wrapped.extend(std::iter::once(TokenTree::Group(
                                        proc_macro2::Group::new(Delimiter::Bracket, target),
                                    )));
                                    target = wrapped;
                                }
                                simple_targets.push(TargetItem::Single(target));
                            }
                        }
                        if any_expanded {
                            continue;
                        }
                    }
                }
            }
            // caret at top level → recursive expand
            if split_by_caret(&tv).is_some() {
                match expand_caret(tv.into_iter().collect(), types, tr, assoc, body.clone(), span) {
                    ParseResult::Ok(sub) => specs.extend(sub),
                    ParseResult::Err(e) => return ParseResult::Err(e),
                }
                continue;
            }
        }
        simple_targets.push(t.clone());
    }

    // 如果 prefixes 为空（只有 unsafe 前缀被移除），直接使用 targets
    if prefixes.is_empty() {
        for t in &simple_targets {
            let ts: TokenStream2 = match t {
                TargetItem::Single(ts) => ts.clone(),
                TargetItem::Multi(_) => {
                    // Multi 只出现在 Container 前缀场景，这里不应该出现
                    return ParseResult::Err(err(span, "unsafe^ 后不能直接跟多参目标如 <X,Y>"));
                }
            };
            specs.push(ImplSpec {
                type_params: types.to_vec(),
                trait_params: tr.clone(),
                assoc_bindings: assoc.to_vec(),
                target: ts,
                custom_body: body.clone(),
                is_unsafe: false,
            });
        }
    } else {
        for p in &prefixes {
            if let PrefixItem::Tuple { elem, bound } = p {
                for t in &simple_targets {
                    // 尝试解析为数字/范围（原有行为：生成多个元组）
                    if let Ok((start, count)) = parse_tuple_count(t) {
                        specs.extend(generate_tuples(
                            elem,
                            bound,
                            start,
                            count,
                            types,
                            tr,
                            body.clone(),
                            suffix,
                        ));
                    } else {
                        // 目标是类型，追加到元组末尾
                        // ()^T = (T), (A,)^T = (A, T), (A,B)^T = (A, B, T)
                        let target_ts = match t {
                            TargetItem::Single(ts) => ts.clone(),
                            TargetItem::Multi(args) => {
                                let mut ts = TokenStream2::new();
                                ts.extend(std::iter::once(TokenTree::Group(
                                    proc_macro2::Group::new(Delimiter::Parenthesis, {
                                        let mut inner = TokenStream2::new();
                                        for (i, arg) in args.iter().enumerate() {
                                            if i > 0 {
                                                inner.extend(std::iter::once(TokenTree::Punct(
                                                    proc_macro2::Punct::new(
                                                        ',',
                                                        proc_macro2::Spacing::Alone,
                                                    ),
                                                )));
                                            }
                                            inner.extend(arg.clone());
                                        }
                                        inner
                                    }),
                                )));
                                ts
                            }
                        };
                        let new_target = match elem {
                            // ()^T = (T,)
                            None => {
                                let mut ts = TokenStream2::new();
                                ts.extend(std::iter::once(TokenTree::Group(
                                    proc_macro2::Group::new(Delimiter::Parenthesis, {
                                        let mut inner = target_ts;
                                        inner.extend(std::iter::once(TokenTree::Punct(
                                            proc_macro2::Punct::new(
                                                ',',
                                                proc_macro2::Spacing::Alone,
                                            ),
                                        )));
                                        inner
                                    }),
                                )));
                                ts
                            }
                            Some(e) => {
                                // (A,)^T = (A, T) 或 (A, B)^T = (A, B, T)
                                let mut ts = TokenStream2::new();
                                ts.extend(std::iter::once(TokenTree::Group(
                                    proc_macro2::Group::new(Delimiter::Parenthesis, {
                                        let mut inner = e.clone();
                                        inner.extend(std::iter::once(TokenTree::Punct(
                                            proc_macro2::Punct::new(
                                                ',',
                                                proc_macro2::Spacing::Alone,
                                            ),
                                        )));
                                        inner.extend(target_ts);
                                        inner
                                    }),
                                )));
                                ts
                            }
                        };
                        let mut all_types = types.to_vec();
                        if let Some(b) = bound {
                            all_types.extend(b.clone().into_iter().map(|tt| {
                                let mut ts = TokenStream2::new();
                                ts.extend(std::iter::once(tt));
                                ts
                            }));
                        }
                        specs.push(ImplSpec {
                            type_params: all_types,
                            trait_params: tr.clone(),
                            assoc_bindings: assoc.to_vec(),
                            target: new_target,
                            custom_body: body.clone(),
                            is_unsafe: false,
                        });
                    }
                }
            } else {
                for t in &simple_targets {
                    let ts = match apply_caret(p, t) {
                        Ok(ts) => ts,
                        Err(e) => return ParseResult::Err(err(span, &e)),
                    };
                    specs.push(ImplSpec {
                        type_params: types.to_vec(),
                        trait_params: tr.clone(),
                        assoc_bindings: assoc.to_vec(),
                        target: ts,
                        custom_body: body.clone(),
                        is_unsafe: false,
                    });
                }
            }
        }
    }
    // 如果有 unsafe 前缀，标记所有 spec
    if has_unsafe {
        for spec in &mut specs {
            spec.is_unsafe = true;
        }
    }
    ParseResult::Ok(specs)
}

pub fn expand_targets_recursive(
    items: Vec<TargetItem>,
    span: Span,
) -> crate::core::types::PResult<Vec<TargetItem>> {
    let mut result = Vec::new();
    for item in items {
        match item {
            TargetItem::Single(ts) => {
                result.extend(expand_single(ts, span)?);
            }
            other => result.push(other),
        }
    }
    Ok(result)
}

pub fn expand_single(ts: TokenStream2, span: Span) -> crate::core::types::PResult<Vec<TargetItem>> {
    let _guard = RecursionGuard::new()?;
    let tokens: Vec<TokenTree> = ts.clone().into_iter().collect();

    // 顶层 ^
    if let Some((left, right)) = split_by_caret(&tokens) {
        let prefixes = parse_prefix_items(&left).map_err(|e| ParseResult::Err(err(span, &e)))?;
        // 如果前缀中有 Tuple，整体交给 expand_caret 递归处理
        if prefixes
            .iter()
            .any(|p| matches!(p, PrefixItem::Tuple { .. }))
        {
            return Ok(vec![TargetItem::Single(ts)]);
        }
        let targets = parse_target_items(&right).map_err(|e| ParseResult::Err(err(span, &e)))?;
        let targets = expand_targets_recursive(targets, span)?;
        let mut r = Vec::new();
        for p in &prefixes {
            for t in &targets {
                let ts = apply_caret(p, t).map_err(|e| ParseResult::Err(err(span, &e)))?;
                r.push(TargetItem::Single(ts));
            }
        }
        return Ok(r);
    }

    // 顶层 - (左结合元组构建)
    if crate::core::dash::split_by_dash(&tokens).is_some() {
        match crate::core::dash::expand_dash(ts, &[], &None, &[], None, span) {
            ParseResult::Ok(specs) => {
                return Ok(specs
                    .into_iter()
                    .map(|s| TargetItem::Single(s.target))
                    .collect());
            }
            ParseResult::Err(e) => return Err(ParseResult::Err(e)),
        }
    }

    // 穿透 [] 和 {}
    if let [TokenTree::Group(ref g)] = tokens.as_slice() {
        if g.delimiter() == Delimiter::Bracket || g.delimiter() == Delimiter::Brace {
            let inner: TokenStream2 = g.stream();

            // bracket 内含逗号 -> 逗号分隔的并列列表，按项分别展开
            if g.delimiter() == Delimiter::Bracket && has_top_level_char(&inner, ',') {
                let has_caret = has_top_level_char(&inner, '^');
                let (segs, _) = split_raw(inner, ',');
                // 如果含 ^，保留每个 segment 的 bracket 包装（Tuple ^ 退化场景）
                if has_caret {
                    let mut results = Vec::new();
                    for seg in segs {
                        let mut wrapped = TokenStream2::new();
                        wrapped.extend(std::iter::once(TokenTree::Group(proc_macro2::Group::new(
                            Delimiter::Bracket,
                            seg,
                        ))));
                        results.push(TargetItem::Single(wrapped));
                    }
                    return Ok(results);
                }
                let mut results = Vec::new();
                for seg in segs {
                    results.extend(expand_single(seg, span)?);
                }
                return Ok(results);
            }

            let expanded = expand_single(inner, span)?;
            let mut r = Vec::new();
            for item in expanded {
                if let TargetItem::Single(inner_ts) = item {
                    let mut wrapped = TokenStream2::new();
                    wrapped.extend(std::iter::once(TokenTree::Group(proc_macro2::Group::new(
                        g.delimiter(),
                        inner_ts,
                    ))));
                    r.push(TargetItem::Single(wrapped));
                } else {
                    r.push(item);
                }
            }
            return Ok(r);
        }
    }

    Ok(vec![TargetItem::Single(ts)])
}
