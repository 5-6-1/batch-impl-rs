//! trait 泛型 bound 继承的真相源：从 trait 定义提取形参映射。
//!
//! 供 codegen 对**未写 bound 的 impl 泛型参数**按位置 + 同名继承
//! （`trait Foo<T: Clone>` + `<T> Foo<T>` → `impl<T: Clone>`）。
//!
//! trait 级 where 子句的**单一形参谓词**（`trait Foo<T> where T: Clone`）
//! 合并进对应位置的 bound（内联 + where 拼接），`A<>` 照抄同样带上；
//! 复合谓词（`Vec<T>: Clone`、`Self: ...`）保守跳过，请手写。

use proc_macro2::{TokenStream, TokenTree};
use quote::quote;
use syn::ItemTrait;

/// trait 形参：名字 + 合并后的 bound（内联 + where 谓词）+ bound 引用的形参名
/// （token 级保守检测）。
#[derive(Default)]
pub(crate) struct TraitParam {
    pub(crate) name: String,
    pub(crate) bound: Option<TokenStream>,
    pub(crate) refs: Vec<String>,
}

/// trait 泛型形参列表（按位置对应 spec 中的 trait 实参），供 codegen 对
/// **未写 bound 的 impl 泛型参数**按位置 + 同名继承。
///
/// 自动化只认同名（`A<>` 照抄 / `<T> A<T>` 同名继承）：
/// - impl 参数按"名字在 trait 实参中的位置"对应形参；形参有 bound 且同名 → 继承；
/// - 异名 → `compile_error!`（请改名或手写 bound）；
/// - 继承的 bound 引用其他形参名（`T: 'a` 的 `'a`、`U: Vec<T>` 的 `T`）而 impl
///   未声明同名 → `compile_error!`（请声明同名或手写）。
///
/// 写 bound = 用户负责，宏不干预（sub trait 蕴含（`trait B: A` 使 `T: B`
/// 隐含 `T: A`）宏无法推理）。trait 级 where 子句仅**单一形参谓词**继承。
#[derive(Default)]
pub(crate) struct TraitBounds {
    pub(crate) params: Vec<TraitParam>,
}

pub(crate) fn extract_trait_bounds(trait_item: &ItemTrait) -> TraitBounds {
    // 形参名集合（类型 + const 为 Ident，生命周期带 `'` 前缀）
    let type_const_names: Vec<String> = trait_item
        .generics
        .params
        .iter()
        .filter_map(|p| match p {
            syn::GenericParam::Type(tp) => Some(tp.ident.to_string()),
            syn::GenericParam::Const(cp) => Some(cp.ident.to_string()),
            _ => None,
        })
        .collect();
    let lt_names: Vec<String> = trait_item
        .generics
        .params
        .iter()
        .filter_map(|p| match p {
            syn::GenericParam::Lifetime(ld) => {
                Some(format!("'{}", ld.lifetime.ident))
            }
            _ => None,
        })
        .collect();
    let mut params = vec![];
    for p in &trait_item.generics.params {
        match p {
            syn::GenericParam::Type(tp) => {
                let bound = if tp.bounds.is_empty() {
                    None
                } else {
                    // 注意：quote 插值只支持 `#ident`，不支持字段访问
                    // `#tp.bounds`（会把 `.bounds` 当字面量输出）
                    let b = &tp.bounds;
                    Some(quote!(#b))
                };
                let refs = bound
                    .as_ref()
                    .map(|b| bound_refs(b, &type_const_names, &lt_names))
                    .unwrap_or_default();
                params.push(TraitParam { name: tp.ident.to_string(), bound, refs });
            }
            syn::GenericParam::Lifetime(ld) => params.push(TraitParam {
                name: format!("'{}", ld.lifetime.ident),
                bound: None,
                refs: vec![],
            }),
            syn::GenericParam::Const(cp) => params.push(TraitParam {
                name: cp.ident.to_string(),
                bound: None,
                refs: vec![],
            }),
        }
    }
    // trait 级 where 子句：单一形参谓词（`X: Bound`）合并进对应位置的 bound
    // （内联 + where 拼接），refs 同步合并；复合谓词保守跳过。
    if let Some(wc) = &trait_item.generics.where_clause {
        for pred in &wc.predicates {
            let syn::WherePredicate::Type(pt) = pred else {
                continue;
            };
            let Some(name) = single_ident_param(&pt.bounded_ty) else {
                continue;
            };
            let Some(pos) = params.iter().position(|p| p.name == name) else {
                continue;
            };
            let b = &pt.bounds;
            let extra = quote!(#b);
            let extra_refs = bound_refs(&extra, &type_const_names, &lt_names);
            let param = &mut params[pos];
            param.bound = Some(match &param.bound {
                Some(inline) => quote!(#inline + #extra),
                None => extra,
            });
            param.refs.extend(extra_refs);
        }
    }
    TraitBounds { params }
}

/// 谓词左侧是否为单一形参名（`T`：无路径、无泛型实参）；返回名字。
fn single_ident_param(ty: &syn::Type) -> Option<String> {
    let syn::Type::Path(tp) = ty else { return None };
    if tp.qself.is_some() {
        return None;
    }
    let seg = tp.path.segments.first()?;
    if tp.path.segments.len() == 1
        && matches!(&seg.arguments, syn::PathArguments::None)
    {
        Some(seg.ident.to_string())
    } else {
        None
    }
}

/// 保守的 bound 形参引用检测：收集 bound token 中出现的形参名。
/// 宁可误报（HRTB 局部名与形参撞名等）——误报只导致"拒绝自动继承、引导手写"，
/// 绝不生成引用错误名字的代码。
fn bound_refs(
    bound: &TokenStream, type_const_names: &[String], lt_names: &[String],
) -> Vec<String> {
    let mut refs = vec![];
    let mut iter = bound.clone().into_iter().peekable();
    while let Some(tt) = iter.next() {
        match tt {
            TokenTree::Ident(id) if type_const_names.contains(&id.to_string()) => {
                refs.push(id.to_string())
            }
            TokenTree::Punct(p) if p.as_char() == '\'' => {
                if let Some(TokenTree::Ident(id)) = iter.peek() {
                    let name = format!("'{}", id);
                    if lt_names.contains(&name) {
                        refs.push(name);
                    }
                }
            }
            _ => {}
        }
    }
    refs
}
