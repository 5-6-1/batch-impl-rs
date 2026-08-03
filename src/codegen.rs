//! 代码生成。
//!
//! 把展开摊平后的叶子 [`Ty`]（见 `lib::parse_batch_trait_entry`）
//! 递归拆解为 [`ImplParts`]（impl 泛型、trait 泛型、关联类型绑定、
//! 目标类型、body、属性、unsafe 标记），再渲染为最终
//! `impl<...> Trait<...> for Target { ... }` 块。
//!
//! v0.4.2 [`extract_impl_parts`] 的 `WithType` 分支由 append 改为
//! prepend：`<A>[<B>T1, <C>T2]` 现输出 `impl<A, B>` / `impl<A, C>`，
//! 与"外层先写"的书写顺序一致。

use crate::TraitBounds;
use crate::diagnostic::compile_error_str;
use crate::types::*;
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

/// 从 Ty 中递归提取 impl 块所需的各部分。
///
/// `impl_spec` 的 AST 节点是按修饰顺序嵌套的（如 `<T> Trait<T> unsafe Box<T> { body }`），
/// 此函数沿树递归拆解，收集：impl 泛型、trait 泛型、关联类型绑定、目标类型、body、属性、unsafe 标记。
pub(crate) struct ImplParts {
    pub(crate) impl_generics: Vec<(TokenStream, Option<Ty>)>,
    pub(crate) trait_generic_names: Vec<TokenStream>,
    pub(crate) associated_types: Vec<(TokenStream, TokenStream)>,
    pub(crate) target_type: Ty,
    pub(crate) body: Option<TokenStream>,
    pub(crate) attrs: Vec<TokenStream>,
    pub(crate) is_unsafe_impl: bool,
    /// 来自 `where{...}` 后缀的 where 谓词列表，多条会被拼接为
    /// `where P1, P2, ...`。元素间以逗号连接。
    pub(crate) where_clauses: Vec<TokenStream>,
}

impl ImplParts {
    /// 叶子节点：无任何修饰，仅目标类型
    fn leaf(target_type: Ty) -> Self {
        ImplParts {
            impl_generics: vec![],
            trait_generic_names: vec![],
            associated_types: vec![],
            target_type,
            body: None,
            attrs: vec![],
            is_unsafe_impl: false,
            where_clauses: vec![],
        }
    }
}

/// 递归拆解 Ty 树，提取 impl 块所需的全部元数据。
///
/// 每遇到一个包装节点就剥离其贡献（泛型、绑定、属性、unsafe），递归处理内层，
/// 直到遇到叶子节点（纯目标类型）。
pub(crate) fn extract_impl_parts(ty: Ty) -> ImplParts {
    match ty {
        Ty::WithType(wt) => {
            let mut parts = extract_impl_parts(*wt.1);
            let (impl_generics, associated_types) =
                (parts.impl_generics, parts.associated_types);
            parts.impl_generics = wt.0.params;
            parts.associated_types = wt.0.bindings;
            parts.impl_generics.extend(impl_generics);
            parts.associated_types.extend(associated_types);
            parts
        }
        Ty::WithTrait(wt) => {
            let mut parts = extract_impl_parts(*wt.1);
            parts.trait_generic_names.extend(wt.0.1.params.into_iter().map(|p| p.0));
            parts.associated_types.extend(wt.0.1.bindings);
            parts
        }
        Ty::WithCode(wc) => match wc.0 {
            Some(inner) => {
                let mut parts = extract_impl_parts(*inner);
                match &mut parts.body {
                    Some(t) => t.extend(wc.1.0),
                    None => parts.body = wc.1.0.into(),
                }
                parts
            }
            // 裸代码块无目标类型，防御性兜底
            None => ImplParts::leaf(wc.into()),
        },
        Ty::WithWhere(ww) => match ww.0 {
            Some(inner) => {
                let mut parts = extract_impl_parts(*inner);
                parts.where_clauses.push(ww.1.0);
                parts
            }
            None => ImplParts::leaf(ww.into()),
        },
        Ty::WithAttr(wa) => match wa.1 {
            Some(inner) => {
                let mut parts = extract_impl_parts(*inner);
                let stream = &wa.0.0;
                parts.attrs.push(quote!(#[#stream]));
                parts
            }
            None => ImplParts::leaf(wa.into()),
        },
        Ty::WithPrefix(wp) => match wp.1 {
            Some(inner) => {
                let mut parts = extract_impl_parts(*inner);
                match wp.0 {
                    // unsafe 前缀 → 标记 unsafe impl
                    TyPrefix::Unsafe => parts.is_unsafe_impl = true,
                    // 引用/指针前缀 → 包到目标类型上
                    _ => {
                        parts.target_type =
                            TyWithPrefix(wp.0, parts.target_type.into()).into()
                    }
                }
                parts
            }
            None => ImplParts::leaf(wp.into()),
        },
        Ty::Error(e) => ImplParts::leaf(e.into()),
        o => ImplParts::leaf(o),
    }
}

/// 递归外提类型中嵌套的 `WithType` 泛型声明（如 `()^N` 的 fresh 泛型元组）。
///
/// 收集 `WithType(<A>, T)` 的参数到 `out`（供 impl 泛型），并把该节点替换为其内层
/// `T`。需要递归到所有容器（Array / Tuple / Group / PrimitiveArray / Generic /
/// WithPrefix / WithTrait / WithCode / WithWhere / WithAttr / Fn）。
fn hoist_type_params(ty: Ty, out: &mut Vec<(TokenStream, Option<Ty>)>) -> Ty {
    match ty {
        Ty::WithType(wt) => {
            out.extend(wt.0.params);
            hoist_type_params(*wt.1, out)
        }
        Ty::Array(a) => {
            TyArray(a.0.into_iter().map(|e| hoist_type_params(e, out)).collect())
                .into()
        }
        Ty::Tuple(t) => {
            TyTuple(t.0.into_iter().map(|e| hoist_type_params(e, out)).collect())
                .into()
        }
        Ty::Group(g) => TyGroup(hoist_type_params(*g.0, out).into()).into(),
        Ty::PrimitiveArray(pa) => {
            TyPrimitiveArray(pa.0.map(|e| hoist_type_params(*e, out).into()), pa.1)
                .into()
        }
        Ty::Generic(g) => {
            let base = hoist_type_params(*g.0, out);
            TyGeneric(base.into(), g.1).into()
        }
        Ty::WithPrefix(wp) => {
            TyWithPrefix(wp.0, wp.1.map(|e| hoist_type_params(*e, out).into())).into()
        }
        Ty::WithTrait(wt) => {
            TyWithTrait(wt.0, hoist_type_params(*wt.1, out).into()).into()
        }
        Ty::WithCode(wc) => {
            TyWithCode(wc.0.map(|e| hoist_type_params(*e, out).into()), wc.1).into()
        }
        Ty::WithWhere(ww) => {
            TyWithWhere(ww.0.map(|e| hoist_type_params(*e, out).into()), ww.1).into()
        }
        Ty::WithAttr(wa) => {
            TyWithAttr(wa.0, wa.1.map(|e| hoist_type_params(*e, out).into())).into()
        }
        Ty::Fn(f) => TyFn(
            f.0.map(|params| {
                params.into_iter().map(|p| hoist_type_params(p, out)).collect()
            }),
            f.1.map(|r| hoist_type_params(*r, out).into()),
            f.2,
        )
        .into(),
        other => other,
    }
}

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
    // 裸代码块：`{...}` 作为整个 spec（开放指令独立成 spec 的退化形态）时，
    // 原样输出其内容为顶层 item（如函数式宏调用 `foo!{...}`），不包进 impl 块。
    if let Ty::WithCode(TyWithCode(None, code)) = &ty {
        return code.0.clone();
    }
    let mut parts = extract_impl_parts(ty);

    // 递归外提目标类型中嵌套的 `WithType`（来自 `()^N` 的 fresh 泛型）：
    // 参数并入 impl 泛型，`WithType(<A>, T)` 替换为 `T`，避免 `<A>` 泄漏在类型中间。
    let mut nested_params = vec![];
    parts.target_type = hoist_type_params(parts.target_type, &mut nested_params);
    parts.impl_generics.extend(nested_params);

    // 继承 trait 泛型 bound：仅对 DSL 未写 bound 的参数（fresh 泛型名不匹配实参，天然跳过）。
    // 自动化只认同名（`A<>` 照抄 / `<T> A<T>` 同名继承），按位置对应：
    // impl 参数名 → 在 trait 实参中的位置 → 该位置的形参 bound。
    // - 异名（实参 `X` 对应形参 `T`）→ compile_error! 引导改名或手写；
    // - 继承的 bound 引用其他形参名（`T: 'a` 的 `'a`）而 impl 未声明同名 → 报错，
    //   绝不生成引用未声明名字的代码。
    let mut errs: Vec<TokenStream> = vec![];
    let trait_args: Vec<String> =
        parts.trait_generic_names.iter().map(|n| n.to_string()).collect();
    let impl_names: std::collections::HashSet<String> =
        parts.impl_generics.iter().map(|(n, _)| n.to_string()).collect();
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
            errs.push(compile_error_str(&format!(
                "batch-impl: trait 实参 `{}` 对应形参 `{}`（bound `{}`），\
                 自动继承要求同名；请改名为 `{}` 或手写 bound",
                key, tp.name, b, tp.name
            )));
            continue;
        }
        if let Some(r) = tp.refs.iter().find(|r| !impl_names.contains(*r)) {
            errs.push(compile_error_str(&format!(
                "batch-impl: 继承的 bound `{}` 引用形参 `{}`，impl 未声明同名参数；\
                 请声明 `{}` 或手写 bound",
                b, r, r
            )));
            continue;
        }
        *bound = Some(Ty::Primitive(TyPrimitive(b.clone())));
    }
    if !errs.is_empty() {
        return errs.into_iter().collect();
    }

    let is_unsafe = is_unsafe_trait || parts.is_unsafe_impl;
    let unsafe_kw = if is_unsafe { quote!(unsafe) } else { quote!() };

    // impl 泛型参数（带 bound）
    let impl_gen = if parts.impl_generics.is_empty() {
        quote!()
    } else {
        let params = parts
            .impl_generics
            .iter()
            .map(|(name, bound)| match bound {
                Some(b) => {
                    let b_tokens = b.to_token_stream();
                    quote!(#name: #b_tokens)
                }
                None => name.clone(),
            })
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

    // where 子句：多条按逗号拼接，无 where 则空
    let where_clause = if parts.where_clauses.is_empty() {
        quote!()
    } else {
        let preds = &parts.where_clauses;
        quote!(where #(#preds),*)
    };

    quote! {
        #(#attrs)*
        #unsafe_kw impl #impl_gen #trait_name #trait_gen for #target #where_clause {
            #(#body_tokens)*
        }
    }
}
