//! impl 元数据拆解：递归提取 impl 块所需各部分（泛型/绑定/属性/unsafe）。

use proc_macro2::TokenStream;
use quote::quote;

use crate::ast::*;

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
pub(crate) fn hoist_type_params(
    ty: Ty, out: &mut Vec<(TokenStream, Option<Ty>)>,
) -> Ty {
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
