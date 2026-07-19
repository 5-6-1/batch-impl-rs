use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use crate::core::types::{ImplSpec, SlotKind};
use crate::core::utils::{has_top_level_char, split_raw};

// ===========================================================================
// 笛卡尔积支持
// ===========================================================================

/// 将 elem 按顶层逗号分割为 slot 列表
pub fn parse_slots(elem: &TokenStream2) -> Vec<SlotKind> {
    let (segs, _) = split_raw(elem.clone(), ',');
    segs.into_iter()
        .map(|seg| {
            let tv: Vec<proc_macro2::TokenTree> = seg.into_iter().collect();
            // 如果以 < 开头，是 bound
            if !tv.is_empty() && matches!(&tv[0], proc_macro2::TokenTree::Punct(p) if p.as_char() == '<') {
                let (result, _) = crate::core::utils::parse_balanced(&tv, 0);
                if let Ok(args) = result {
                    if args.len() == 1 {
                        return SlotKind::Bound(args.into_iter().next().unwrap());
                    }
                }
            }
            SlotKind::Fixed(tv.into_iter().collect())
        })
        .collect()
}

/// 生成长度为 n 的所有笛卡尔积组合
/// 返回 (tuple_ts, extra_types) 列表
/// 上限：1024 种组合，防止组合爆炸
pub const MAX_CARTESIAN_COMBOS: usize = 1024;

pub fn cartesian_combos(slots: &[SlotKind], n: usize, suffix: u64) -> Vec<(TokenStream2, Vec<TokenStream2>)> {
    if n == 0 {
        return vec![(quote! { () }, vec![])];
    }
    let num_slots = slots.len();
    // 总共 num_slots^n 种组合
    let total = num_slots.pow(n as u32);
    if total > MAX_CARTESIAN_COMBOS {
        // 超过上限，返回空（调用方应检查并报错）
        return vec![];
    }
    let mut results = Vec::with_capacity(total);
    for mut idx in 0..total {
        let mut elems: Vec<TokenStream2> = Vec::with_capacity(n);
        let mut extra_types: Vec<TokenStream2> = Vec::new();
        let mut generic_counter = 0usize;
        for _ in 0..n {
            let slot_idx = idx % num_slots;
            idx /= num_slots;
            match &slots[slot_idx] {
                SlotKind::Fixed(ts) => {
                    elems.push(ts.clone());
                }
                SlotKind::Bound(b) => {
                    let letter = generic_letter(generic_counter, suffix);
                    generic_counter += 1;
                    extra_types.push(quote! { #letter: #b });
                    elems.push(quote! { #letter });
                }
            }
        }
        let ts = if n == 1 {
            quote! { ( #(#elems),* ,) }
        } else {
            quote! { ( #(#elems),* ) }
        };
        results.push((ts, extra_types));
    }
    results
}

/// 生成泛型名：A_7f3a_, B_7f3a_, ... 基于 Span 位置哈希
pub fn generic_letter(i: usize, suffix: u64) -> proc_macro2::Ident {
    let letters = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let letter = if i < 26 {
        &letters[i..i + 1]
    } else {
        &format!("{}{}", &letters[i % 26..i % 26 + 1], i / 26)
    };
    proc_macro2::Ident::new(&format!("{}_{:x}_", letter, suffix), Span::mixed_site())
}

/// 解析元组计数："5" → (0,5), "1..5" → (1,4), "1..=5" → (1,5)
pub fn parse_tuple_count(t: &crate::core::types::TargetItem) -> Result<(usize, usize), String> {
    match t {
        crate::core::types::TargetItem::Single(ts) => {
            let tokens: Vec<proc_macro2::TokenTree> = ts.clone().into_iter().collect();
            // 尝试解析为 M..N 或 M..=N
            if tokens.len() >= 3 {
                // 找第一个 .. 或 ..=
                let mut dot_start = None;
                let mut inclusive = false;
                let mut i = 0;
                while i + 1 < tokens.len() {
                    if matches!(&tokens[i], proc_macro2::TokenTree::Punct(p) if p.as_char() == '.')
                        && matches!(&tokens[i+1], proc_macro2::TokenTree::Punct(p) if p.as_char() == '.')
                    {
                        dot_start = Some(i);
                        if i + 2 < tokens.len()
                            && matches!(&tokens[i+2], proc_macro2::TokenTree::Punct(p) if p.as_char() == '=')
                        {
                            inclusive = true;
                        }
                        break;
                    }
                    i += 1;
                }
                if let Some(d) = dot_start {
                    let left: String = tokens[..d]
                        .iter()
                        .map(|t| t.to_string())
                        .collect::<String>();
                    let right_start = if inclusive { d + 3 } else { d + 2 };
                    let right: String = tokens[right_start..]
                        .iter()
                        .map(|t| t.to_string())
                        .collect::<String>();
                    let start: usize = left
                        .trim()
                        .parse()
                        .map_err(|_| format!("范围起始应为非负整数，得到 `{}`", left))?;
                    let end: usize = right
                        .trim()
                        .parse()
                        .map_err(|_| format!("范围结束应为非负整数，得到 `{}`", right))?;
                    if inclusive {
                        if end < start {
                            return Err(format!(
                                "范围 {}..={} 无效：结束值 {} 不小于起始值 {}",
                                start, end, end, start
                            ));
                        }
                        return Ok((start, end - start + 1));
                    } else {
                        if end <= start {
                            return Err(format!(
                                "范围 {}..{} 无效：结束值 {} 不大于起始值 {}",
                                start, end, end, start
                            ));
                        }
                        return Ok((start, end - start));
                    }
                }
            }
            // 单个整数
            let s = ts.to_string().trim().to_string();
            let n: usize = s
                .parse()
                .map_err(|_| format!("元组长度应为非负整数，得到 `{}`", s))?;
            Ok((0, n))
        }
        _ => Err("元组 ^ 右侧应为整数(如 ^3)或范围(如 ^1..5, ^1..=5)".into()),
    }
}

/// 为元组前缀生成多个 ImplSpec
pub fn generate_tuples(
    elem: &Option<TokenStream2>,
    bound: &Option<TokenStream2>,
    start: usize,
    count: usize,
    parent_types: &[TokenStream2],
    parent_trait: &Option<Vec<TokenStream2>>,
    body: Option<TokenStream2>,
    suffix: u64,
) -> Vec<ImplSpec> {
    let mut specs = Vec::new();
    // 检查 elem 是否包含多个类型（逗号分隔）→ 笛卡尔积模式
    let slots: Option<Vec<SlotKind>> = elem.as_ref().and_then(|e| {
        if has_top_level_char(e, ',') {
            Some(parse_slots(e))
        } else {
            None
        }
    });

    for n in start..start + count {
        if let Some(ref slots) = slots {
            // 笛卡尔积模式：每个位置从 slots 中取一个
            let combos = cartesian_combos(slots, n, suffix);
            if combos.is_empty() && n > 0 {
                // 超过上限，报错
                let num_slots = slots.len();
                let total = num_slots.pow(n as u32);
                // 使用 compile_error! 报错
                let msg = format!(
                    "笛卡尔积组合数 {} ({}^{}) 超过上限 {}，请减少类型数量或长度",
                    total, num_slots, n, MAX_CARTESIAN_COMBOS
                );
                let lit = proc_macro2::Literal::string(&msg);
                specs.push(ImplSpec {
                    type_params: vec![],
                    trait_params: None,
                    assoc_bindings: vec![],
                    target: quote::quote_spanned! { Span::call_site() => ::core::compile_error!(#lit) },
                    custom_body: None,
                    is_unsafe: false,
                });
                return specs;
            }
            for (tuple_ts, extra_types) in combos {
                let mut all_types = parent_types.to_vec();
                all_types.extend(extra_types);
                specs.push(ImplSpec {
                    type_params: all_types,
                    trait_params: parent_trait.clone(),
                    assoc_bindings: vec![],
                    target: tuple_ts,
                    custom_body: body.clone(),
                    is_unsafe: false,
                });
            }
        } else {
            // 原有模式
            let (tuple_ts, extra_types): (TokenStream2, Vec<TokenStream2>) = if n == 0 {
                (quote! { () }, vec![])
            } else {
                match (elem, bound) {
                    (Some(e), _) => {
                        let elems: Vec<_> = (0..n).map(|_| e.clone()).collect();
                        let ts = if n == 1 {
                            quote! { ( #(#elems),* ,) }
                        } else {
                            quote! { ( #(#elems),* ) }
                        };
                        (ts, vec![])
                    }
                    (None, Some(b)) => {
                        let mut extras = vec![];
                        let elems: Vec<TokenStream2> = (0..n)
                            .map(|i| {
                                let letter = generic_letter(i, suffix);
                                extras.push(quote! { #letter: #b });
                                quote! { #letter }
                            })
                            .collect();
                        let ts = if n == 1 {
                            quote! { ( #(#elems),* ,) }
                        } else {
                            quote! { ( #(#elems),* ) }
                        };
                        (ts, extras)
                    }
                    (None, None) => {
                        let mut extras = vec![];
                        let elems: Vec<TokenStream2> = (0..n)
                            .map(|i| {
                                let letter = generic_letter(i, suffix);
                                extras.push(quote! { #letter });
                                quote! { #letter }
                            })
                            .collect();
                        let ts = if n == 1 {
                            quote! { ( #(#elems),* ,) }
                        } else {
                            quote! { ( #(#elems),* ) }
                        };
                        (ts, extras)
                    }
                }
            };
            let mut all_types = parent_types.to_vec();
            all_types.extend(extra_types);
            specs.push(ImplSpec {
                type_params: all_types,
                trait_params: parent_trait.clone(),
                assoc_bindings: vec![],
                target: tuple_ts,
                custom_body: body.clone(),
                is_unsafe: false,
            });
        }
    }
    specs
}
