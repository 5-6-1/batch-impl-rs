//! 生成层：impl 块代码生成。
//!
//! 把展开摊平后的叶子 [`Ty`]（见 `lib::parse_batch_trait_entry`）
//! 递归拆解为 [`ImplParts`]（impl 泛型、trait 泛型、关联类型绑定、
//! 目标类型、body、属性、unsafe 标记），再渲染为最终
//! `impl<...> Trait<...> for Target { ... }` 块。

mod impl_parts;
pub(crate) use impl_parts::*;

use crate::TraitBounds;
use crate::ast::types_render::render_param;
use crate::ast::*;
use crate::util::{compile_err, compile_error_str};
use proc_macro2::{TokenStream, TokenTree};
use quote::quote;

/// 生成一个 impl 块（对摊平后的单个叶子 `Ty`）。
///
/// `trait_bounds`：trait 泛型形参列表（按位置对应 spec 中的 trait 实参）。
/// 对**未写 bound** 的 impl 泛型参数按位置 + 同名继承（`trait Foo<T: Clone>` +
/// `<T> Foo<T>` → `impl<T: Clone>`）；异名 / bound 引用未声明形参名 → 报错，
/// 用户已写 bound 的参数不干预（sub trait 蕴含宏无法推理，写了 = 用户负责）。
///
/// 三个出口：
/// - `Ty::Error` → 直接输出 `compile_error!` 流；
/// - 裸代码块 `WithCode(None, ...)`（开放指令扩展产物）→ 原样作为顶层 item 注入，
///   不包进 impl；
/// - 其余 → 拆解元数据（`extract_impl_parts`）→ 嵌套泛型外提
///   （`hoist_type_params`）→ 构建泛型参数 / trait 泛型 / impl body → 渲染 `quote!` 块。
pub(crate) fn generate_impl(
    ty: Ty, trait_name: &TokenStream, is_unsafe_trait: bool,
    trait_bounds: &TraitBounds,
) -> TokenStream {
    if let Ty::Error(e) = ty {
        return e.0;
    }
    // 裸代码块：`{...}` 作为整个 spec 时原样输出为顶层 item（不包进 impl 块）
    if let Ty::WithCode(TyWithCode(None, code)) = &ty {
        return code.0.clone();
    }
    let mut parts = extract_impl_parts(ty);

    // 递归外提目标类型中嵌套的 `WithType`（fresh 泛型），避免 `<A>` 泄漏
    let mut nested_params = vec![];
    parts.target_type = hoist_type_params(parts.target_type, &mut nested_params);
    parts.impl_generics.extend(nested_params);

    // 继承 trait 泛型 bound：同名继承/异名报错规则见 trait_bounds 模块文档。
    let mut errs: Vec<TokenStream> = vec![];
    let trait_args: Vec<String> =
        parts.trait_generic_names.iter().map(|n| n.to_string()).collect();
    // const 形参在 parse 层的名字是 `const N`（渲染 `const N: usize` 需要关键字），
    // 归一为 `N` 以匹配 trait 实参与 where 谓词引用
    let impl_name_streams: Vec<TokenStream> = parts
        .impl_generics
        .iter()
        .map(|(n, _)| {
            let s = n.to_string();
            let bare = s.strip_prefix("const ").unwrap_or(&s);
            bare.parse().unwrap()
        })
        .collect();
    let impl_names: std::collections::HashSet<String> =
        impl_name_streams.iter().map(|n| n.to_string()).collect();
    for (name, bound) in &mut parts.impl_generics {
        if bound.is_some() {
            continue;
        }
        let key = name.to_string();
        // 该参数作为 trait 实参出现的位置（未出现 = 与 trait 无关，不继承）
        let Some(pos) = trait_args.iter().position(|a| a == &key) else {
            continue;
        };
        let Some(tp) = trait_bounds.params.get(pos) else {
            continue;
        };
        let Some(b) = &tp.bound else {
            continue;
        };
        if tp.name != key {
            errs.push(compile_err!(
                "batch-impl: trait 实参 `{}` 对应形参 `{}`（bound `{}`），\
                 自动继承要求同名；请改名为 `{}` 或手写 bound",
                key,
                tp.name,
                b,
                tp.name
            ));
            continue;
        }
        if let Some(r) = tp.refs.iter().find(|r| !impl_names.contains(*r)) {
            errs.push(compile_err!(
                "batch-impl: 继承的 bound `{}` 引用形参 `{}`，impl 未声明同名参数；\
                 请声明 `{}` 或手写 bound",
                b,
                r,
                r
            ));
            continue;
        }
        *bound = Some(Ty::Primitive(TyPrimitive(b.clone())));
    }
    // 未合并的 where 谓词（复合谓词 / 生命周期谓词）：引用检查后附加到 impl where
    for (pred, refs) in &trait_bounds.extra_predicates {
        if let Some(r) = refs.iter().find(|r| !impl_names.contains(*r)) {
            errs.push(compile_err!(
                "batch-impl: 继承的 where 谓词 `{}` 引用形参 `{}`，\
                 impl 未声明同名参数；请声明 `{}` 或手写 where",
                pred,
                r,
                r
            ));
            continue;
        }
        parts.where_clauses.push(pred.clone());
    }
    // where 谓词宏元层替换（`@N` → impl 泛型第 N 位、`@trait` → trait 名）
    let mut where_resolved: Vec<TokenStream> = vec![];
    for pred in &parts.where_clauses {
        match resolve_where_at(pred, &impl_name_streams, trait_name) {
            Ok(p) => where_resolved.push(p),
            Err(e) => errs.push(e),
        }
    }
    if !errs.is_empty() {
        return errs.into_iter().collect();
    }
    let parts = parts; // 后续只用 where_resolved，不再改 parts

    let is_unsafe = is_unsafe_trait || parts.is_unsafe_impl;
    let unsafe_kw = if is_unsafe { quote!(unsafe) } else { quote!() };

    // impl 泛型参数（带 bound）
    let impl_gen = if parts.impl_generics.is_empty() {
        quote!()
    } else {
        let params = parts
            .impl_generics
            .iter()
            .map(|(name, bound)| render_param(name, bound.as_ref()))
            .collect::<Vec<_>>();
        quote!(<#(#params),*>)
    };

    // trait 泛型参数（仅名字）
    let trait_gen = if parts.trait_generic_names.is_empty() {
        quote!()
    } else {
        let names = &parts.trait_generic_names;
        quote!(<#(#names),*>)
    };

    // 目标类型
    let target = &parts.target_type;

    // impl body：关联类型 + 用户 body
    let mut body_tokens = vec![];
    for (name, value) in &parts.associated_types {
        body_tokens.push(quote!(type #name = #value;));
    }
    if let Some(body) = &parts.body {
        body_tokens.push(body.clone());
    }

    // 属性
    let attrs = parts.attrs;

    // where 子句：多条按逗号拼接，无 where 则空（谓词已由 resolve_where_at 替换）
    let where_clause = if where_resolved.is_empty() {
        quote!()
    } else {
        let preds = &where_resolved;
        quote!(where #(#preds),*)
    };

    quote! {
        #(#attrs)*
        #unsafe_kw impl #impl_gen #trait_name #trait_gen for #target #where_clause {
            #(#body_tokens)*
        }
    }
}

/// where 谓词中的宏元层位置引用：`@N` → impl 泛型第 N 位名字、`@trait` → trait 名。
/// `@N` 越界或 `@` 后非位置数字/`@trait` 报错。blanket 包装 where 已预替换，此处
/// 只处理用户 where 谓词（元组/普通 spec——`()^2 where{@0: Clone}`、`<T> where{@0: X}`）。
fn resolve_where_at(
    pred: &TokenStream, impl_names: &[TokenStream], trait_name: &TokenStream,
) -> Result<TokenStream, TokenStream> {
    let tokens: Vec<_> = pred.clone().into_iter().collect();
    let mut out = vec![];
    let mut i = 0;
    while i < tokens.len() {
        if let TokenTree::Punct(p) = &tokens[i]
            && p.as_char() == '@'
        {
            match tokens.get(i + 1) {
                Some(TokenTree::Literal(lit)) => {
                    let idx: usize = lit.to_string().parse().map_err(|_| {
                        compile_error_str("batch-impl: where 谓词中 `@` 后必须是位置数字（如 `@0`）")
                    })?;
                    let Some(name) = impl_names.get(idx) else {
                        return Err(compile_err!(
                            "batch-impl: where 谓词中 `@{}` 越界（impl 泛型共 {} 个，索引从 0 起）",
                            idx,
                            impl_names.len()
                        ));
                    };
                    out.extend(name.clone());
                    i += 2;
                }
                Some(TokenTree::Ident(id)) if id == "trait" => {
                    out.extend(trait_name.clone());
                    i += 2;
                }
                _ => {
                    return Err(compile_error_str(
                        "batch-impl: where 谓词中 `@` 后必须是位置数字（如 `@0`）或 `@trait`",
                    ));
                }
            }
        } else {
            out.push(tokens[i].clone());
            i += 1;
        }
    }
    Ok(out.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze::extract_trait_bounds;
    use syn::parse_quote;

    /// `WhereArr<>` 展开场景：impl 泛型 `[T, const N: usize]`（parse 层名字
    /// 为 `const N`，渲染需关键字）、trait 实参 `[T, N]`、谓词 `[T; N]: Sized`
    /// 引用 N——归一化后检查通过，展开无 compile_error（防 IDE/旧产物误报回归）
    #[test]
    fn const_param_where_predicate_no_error() {
        let trait_def: syn::ItemTrait = parse_quote!(
            trait WhereArr<T, const N: usize>
            where
                [T; N]: Sized,
            {
            }
        );
        let tb = extract_trait_bounds(&trait_def);
        let target: Ty = TyTuple(vec![]).into();
        let trait_ty = TyTrait(
            quote!(WhereArr),
            TyTypeParam {
                params: vec![(quote!(T), None), (quote!(N), None)],
                bindings: vec![],
            },
        );
        let wrapped = TyWithTrait(trait_ty, target.into());
        let impl_ty = TyWithType(
            TyTypeParam {
                params: vec![
                    (quote!(T), None),
                    (
                        quote!(const N),
                        Some(Ty::Primitive(TyPrimitive(quote!(usize)))),
                    ),
                ],
                bindings: vec![],
            },
            wrapped.into(),
        )
        .into();
        let out = generate_impl(impl_ty, &quote!(WhereArr), false, &tb).to_string();
        assert!(
            !out.contains("compile_error"),
            "展开不应含 compile_error：{out}"
        );
        assert!(
            out.contains("where [T ; N] : Sized"),
            "缺少 where 谓词：{out}"
        );
        assert!(
            out.contains("impl < T , const N : usize > WhereArr < T , N >"),
            "impl 泛型异常：{out}"
        );
    }
}
