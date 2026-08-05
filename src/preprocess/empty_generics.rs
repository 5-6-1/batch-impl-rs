//! `A<>` / `A<绑定们>` 预处理：trait 泛型照抄。
//!
//! 在指令预处理与 where 改写之后、DSL 解析之前，扫描顶层 token 流中的
//! `Ident` + 尖括号组（`angle_collect` 配对产物，空实参或纯绑定实参），
//! 展开为尖括号组序列——与 `angle_collect` 的配对产物形态一致，
//! parse 层无需区分来源。

use proc_macro2::{TokenStream, TokenTree};
use quote::quote;
use syn::ItemTrait;

use crate::analyze::TraitBounds;
use crate::util::scan_stop;

/// 实参段是否"纯绑定"（`Item = T, K = U`：每个顶层逗号段都含 `=`）。
/// 判定为纯绑定才允许 `A<绑定们>` 照抄展开；含位置参数的 `A<T, Item=U>`
/// 是普通 DSL 语法（不展开，位置参数由用户声明）。
fn args_all_bindings(args: &[TokenTree]) -> bool {
    let mut rest = args;
    while let Some(idx) = scan_stop(rest, &[',']) {
        // 段必须含顶层 `=`（绑定）
        if scan_stop(&rest[..idx], &['=']).is_none() {
            return false;
        }
        rest = &rest[idx + 1..];
    }
    scan_stop(rest, &['=']).is_some()
}

/// 渲染 `A<>` 的形参段：类型形参用 [`TraitBounds`] 合并后的 bound
/// （内联 + where 谓词），生命周期 / const 原样照抄。
fn render_formals(
    trait_def: &ItemTrait, trait_bounds: &TraitBounds,
) -> Vec<TokenStream> {
    let mut formals = vec![];
    for (i, p) in trait_def.generics.params.iter().enumerate() {
        match p {
            syn::GenericParam::Lifetime(_) | syn::GenericParam::Const(_) => {
                formals.push(quote!(#p));
            }
            syn::GenericParam::Type(tp) => {
                let id = &tp.ident;
                match trait_bounds.params.get(i).and_then(|t| t.bound.clone()) {
                    Some(b) => formals.push(quote!(#id: #b)),
                    None => formals.push(quote!(#id)),
                }
            }
        }
    }
    formals
}

/// `A<>` / `A<绑定们>` 预处理：扫描顶层 token 流中的 `Ident` + 尖括号组
/// （`angle_collect` 配对产物，空实参或纯绑定实参），展开为
/// `尖括号组(形参) Ident 尖括号组(实参 + 绑定)`——与 `angle_collect`
/// 的配对产物形态一致，parse 层无需区分来源。
///
/// - 只处理**顶层**的 `Ident` + 尖括号组（`B<A<>>` 嵌套在组内，不展开；
///   含位置参数的 `A<T, Item=U>` 是普通 DSL 语法，不展开）；
/// - trait 无泛型参数时透传（`A<>` 由 DSL 解析为空实参，渲染 `A`）；
/// - 仅 `#[batch_impl]` / `#[batch_impl_only]` 可用（需要 trait 定义渲染形参）；
///   `batch_trait!` 无 trait 定义，`A<>` 原样透传。
pub(crate) fn expand_empty_trait_generics(
    tokens: &[TokenTree], trait_def: &ItemTrait, trait_bounds: &TraitBounds,
) -> Result<Vec<TokenTree>, TokenStream> {
    if trait_def.generics.params.is_empty() {
        return Ok(tokens.to_vec());
    }
    // 预渲染实参名列表（展开时作为尖括号组的实参段）
    let arg_names = crate::analyze::generic_param_names(&trait_def.generics);
    let formals = render_formals(trait_def, trait_bounds);
    let mut out = vec![];
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            // `Ident` + 尖括号组（`angle_collect` 配对产物）——顶层才展开：
            // 空实参（`A<>`）或**纯绑定实参**（`A<Item=T>`）→ 位置实参照抄
            // trait 形参，绑定原样保留；含位置参数的 `A<T, Item=U>` 是普通
            // DSL 语法（不展开）。组内的 `Ident<>`（嵌套如 `B<A<>>`）不处理。
            TokenTree::Ident(id) => {
                let group = match tokens.get(i + 1) {
                    Some(TokenTree::Group(g)) if g.delimiter() == delimiter![<>] => g,
                    _ => {
                        out.push(tokens[i].clone());
                        i += 1;
                        continue;
                    }
                };
                let args: Vec<TokenTree> = group.stream().into_iter().collect();
                let bindings_only = !args.is_empty() && args_all_bindings(&args);
                if args.is_empty() || bindings_only {
                    // 展开为尖括号组序列（`angle_collect` 的配对产物形态）：
                    // `尖括号组(<'a, T: bounds, const N>) A 尖括号组(<'a, T, N, Item = T>)`
                    out.push(
                        proc_macro2::Group::new(
                            delimiter![<>],
                            quote!(#(#formals),*),
                        )
                        .into(),
                    );
                    out.extend(quote!(#id));
                    let args_ts: TokenStream = if args.is_empty() {
                        quote!(#(#arg_names),*)
                    } else {
                        let bind_ts: TokenStream = args.iter().cloned().collect();
                        quote!(#(#arg_names),* , #bind_ts)
                    };
                    out.push(proc_macro2::Group::new(delimiter![<>], args_ts).into());
                    i += 2;
                } else {
                    out.push(tokens[i].clone());
                    i += 1;
                }
            }
            _ => {
                out.push(tokens[i].clone());
                i += 1;
            }
        }
    }
    Ok(out)
}
