//! trait 泛型 bound 继承的真相源：从 trait 定义提取形参映射。
//!
//! 供 codegen 对**未写 bound 的 impl 泛型参数**按位置 + 同名继承
//! （`trait Foo<T: Clone>` + `<T> Foo<T>` → `impl<T: Clone>`）。
//!
//! trait 级 where 子句的处理：
//! - **单一形参谓词**（`trait Foo<T> where T: Clone`，左侧为裸形参名）→
//!   合并进对应位置的 bound（内联 + where 拼接），`A<>` 照抄同样带上；
//! - **其余谓词**（`T::Item: Clone`、`Vec<T>: ...`、生命周期谓词等）→
//!   原样透传存 [`TraitBounds::extra_predicates`]，codegen 附加到 impl 的
//!   where 子句——覆盖全部谓词形态，不丢弃。
//!
//! 引用收集在 **syn AST 上做**（[`syn::visit`]）：单段路径（`T`）与泛型实参
//! （`Vec<T>` 的 `T`）是形参引用位置；`::` 后的路径段（关联类型名）、
//! 关联类型绑定名（`dyn Trait<Item = T>` 的 `Item`）、HRTB binder
//! （`for<'a>` 的 `'a`）天然排除——token 级扫描无法区分这些，AST 可以。

use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::quote;
use syn::ItemTrait;
use syn::visit::{self, Visit};

/// trait 形参：名字 + 合并后的 bound（内联 + where 谓词）+ bound 引用的形参名。
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
/// - 继承的 bound / 谓词引用其他形参名（`T: 'a` 的 `'a`、`A::B` 的 `A`）而 impl
///   未声明同名 → `compile_error!`（请声明同名或手写）。
///
/// 写 bound = 用户负责，宏不干预（sub trait 蕴含（`trait B: A` 使 `T: B`
/// 隐含 `T: A`）宏无法推理）。trait 级 where 子句的单一形参谓词并入 bound，
/// 其余谓词原样透传（[`TraitBounds::extra_predicates`]）。
#[derive(Default)]
pub(crate) struct TraitBounds {
    pub(crate) params: Vec<TraitParam>,
    /// 未合并进 bound 的 where 谓词（复合谓词 / 生命周期谓词）+ 引用的形参名。
    /// codegen 附加到 impl 的 where 子句，并做引用检查（改名场景报错引导）。
    pub(crate) extra_predicates: Vec<(TokenStream, Vec<String>)>,
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
                let refs =
                    collect_bound_refs(&tp.bounds, &type_const_names, &lt_names);
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
    let mut extra_predicates = vec![];
    if let Some(wc) = &trait_item.generics.where_clause {
        for pred in &wc.predicates {
            let tokens = quote!(#pred);
            // 单一形参谓词（`X: Bound`）合并进对应位置的 bound
            if let syn::WherePredicate::Type(pt) = pred
                && let Some(name) = single_ident_param(&pt.bounded_ty)
                && let Some(pos) = params.iter().position(|p| p.name == name)
            {
                // 注意：quote 插值只支持 `#ident`，不支持字段访问
                let b = &pt.bounds;
                let extra = quote!(#b);
                let extra_refs =
                    collect_bound_refs(&pt.bounds, &type_const_names, &lt_names);
                let param = &mut params[pos];
                param.bound = Some(match &param.bound {
                    Some(inline) => quote!(#inline + #extra),
                    None => extra,
                });
                param.refs.extend(extra_refs);
                continue;
            }
            // 其余谓词：原样透传 + 引用收集
            let refs = collect_predicate_refs(pred, &type_const_names, &lt_names);
            extra_predicates.push((tokens, refs));
        }
    }
    TraitBounds { params, extra_predicates }
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

/// 收集 bound 列表（`Clone + Send`、HRTB 等）引用的形参名。
fn collect_bound_refs(
    bounds: &syn::punctuated::Punctuated<syn::TypeParamBound, syn::Token![+]>,
    type_const_names: &[String], lt_names: &[String],
) -> Vec<String> {
    let mut c = Collector::new(type_const_names, lt_names);
    for b in bounds {
        c.visit_type_param_bound(b);
    }
    c.refs
}

/// 收集 where 谓词（左侧类型 + bound）引用的形参名。
fn collect_predicate_refs(
    pred: &syn::WherePredicate, type_const_names: &[String], lt_names: &[String],
) -> Vec<String> {
    let mut c = Collector::new(type_const_names, lt_names);
    c.visit_where_predicate(pred);
    c.refs
}

/// syn AST 引用收集器。
///
/// 精确规则（AST 语义，非 token 猜测）：
/// - 单段路径（`T`）→ ident 是类型名本身，撞形参名即引用；`::` 后的段
///   （关联类型名）与泛型实参（`Vec<T>` 的 `T`）由默认 visit 处理；
/// - HRTB binder（`for<'a>`）压栈，binder 内的 `'a` 不收集；
/// - 关联类型绑定名（`dyn Trait<Item = T>` 的 `Item`）不是类型，不收集。
struct Collector<'a> {
    type_const_names: &'a [String],
    lt_names: &'a [String],
    /// HRTB binder 栈（`for<'a>` 的 `'a` 是局部名，遮蔽外层同名形参）
    hrtb: Vec<HashSet<String>>,
    refs: Vec<String>,
}

impl<'a> Collector<'a> {
    fn new(type_const_names: &'a [String], lt_names: &'a [String]) -> Self {
        Collector { type_const_names, lt_names, hrtb: vec![], refs: vec![] }
    }

    fn in_hrtb(&self, name: &str) -> bool {
        self.hrtb.iter().any(|s| s.contains(name))
    }

    /// 压入 binder 名集合；返回是否压入（供调用方恢复）
    fn push_hrtb(&mut self, lifetimes: Option<&syn::BoundLifetimes>) -> bool {
        if let Some(bl) = lifetimes {
            let set = bl
                .lifetimes
                .iter()
                .filter_map(|p| match p {
                    syn::GenericParam::Lifetime(ld) => {
                        Some(format!("'{}", ld.lifetime.ident))
                    }
                    _ => None,
                })
                .collect();
            self.hrtb.push(set);
            true
        } else {
            false
        }
    }
}

impl<'ast> Visit<'ast> for Collector<'_> {
    fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
        // 单段路径（`T`）：ident 是类型名本身，撞形参名即引用
        if node.qself.is_none()
            && let Some(seg) = node.path.segments.first()
            && node.path.segments.len() == 1
            && matches!(&seg.arguments, syn::PathArguments::None)
            && self.type_const_names.contains(&seg.ident.to_string())
        {
            self.refs.push(seg.ident.to_string());
        }
        // 默认继续：泛型实参（`Vec<T>` 的 T）、qself（`<T as Trait>::Item` 的 T）
        visit::visit_type_path(self, node);
    }

    fn visit_lifetime(&mut self, node: &'ast syn::Lifetime) {
        let name = format!("'{}", node.ident);
        if !self.in_hrtb(&name) && self.lt_names.contains(&name) {
            self.refs.push(name);
        }
    }

    fn visit_expr(&mut self, node: &'ast syn::Expr) {
        // const 泛型实参 / 数组长度（`[T; N]` 的 N、`Foo<N>` 的 N）：
        // 单段路径表达式是 const 形参引用位置（类型形参不能出现在表达式里）
        if let syn::Expr::Path(ep) = node
            && ep.qself.is_none()
            && let Some(seg) = ep.path.segments.first()
            && ep.path.segments.len() == 1
            && matches!(&seg.arguments, syn::PathArguments::None)
            && self.type_const_names.contains(&seg.ident.to_string())
        {
            self.refs.push(seg.ident.to_string());
        }
        visit::visit_expr(self, node);
    }

    fn visit_trait_bound(&mut self, node: &'ast syn::TraitBound) {
        // `for<'a> Fn(&'a u8)`：binder 内的 'a 是局部名，不收集
        let pushed = self.push_hrtb(node.lifetimes.as_ref());
        visit::visit_trait_bound(self, node);
        if pushed {
            self.hrtb.pop();
        }
    }

    fn visit_type_fn_ptr(&mut self, node: &'ast syn::TypeFnPtr) {
        // `fn<'a>(&'a u8)` 类型：binder 同样遮蔽
        let pushed = self.push_hrtb(node.lifetimes.as_ref());
        visit::visit_type_fn_ptr(self, node);
        if pushed {
            self.hrtb.pop();
        }
    }
}
