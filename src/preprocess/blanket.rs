//! `#blanket` 覆盖式委托指令：包装列表解析 + 逐包装生成完整委托 spec。
//!
//! 与 `expand_directive` 的 `Vec<TokenTree>` 返回契约配套：产物为多段完整
//! spec（逗号分隔），只能独立成 spec（自含泛型/目标/委托，见
//! architecture.md「语法域隔离」的附着语义说明）。

use proc_macro2::{Group, TokenStream, TokenTree};
use quote::quote;
use syn::ItemTrait;

use crate::ast::fresh_param;
use crate::preprocess::{
    angle_collect, build_from_item, collect_call_args, get_trait_item,
    parse_names_from_tokens,
};
use crate::util::is_single_colon;
use crate::util::{compile_err, compile_error_str};

/// `#blanket(@all){&,Box,Rc}` — 覆盖式委托：为每个包装类型生成一段完整 spec。
///
/// 等价于对每个包装手写 `<T: Trait> 包装^T #delegate(选中的方法){*…*self}`——
/// 免写包装矩阵与委托体。包装元素是**任意类型表达式**（`&`/`&mut`/`Box`/
/// `Rc`/`Arc`/`MyPtr`/`Box^Arc`/`Cow<'_>` 等），经 `^T` 应用到 fresh 泛型：
/// 目标类型 = 包装表达式 + `^T`（`Box^Arc:2` → `Box<Arc<T>>`、`Cow<'_>` →
/// `Cow<'_, T>`）。**嵌套包装必须用 `^` 链**（`Box^Arc`），`<` 预填是追加
/// 语义（`Box<Arc>^T` = `Box<Arc, T>`，错误）。
///
/// 委托体解引用层数：`:N` 标注（`Box^Arc:2`）或默认 1——`*` 数量 = N + 1
/// （self 是 `&包装<T>`，先解 self 引用再解 N 层包装）。默认恒为 1，宏不猜
/// 包装内部 Deref 层数；嵌套包装须显式 `:N`（写错退化为 rustc 方法不存在
/// 错误，文档已警示）。`*const`/`*mut`（安全代码无法解引用裸指针委托）、
/// `self`（无意义）、空元素/非法 `:N` 报错。
///
/// **泛型 trait**（`trait Foo<X> where X: Clone`）支持：trait 形参照抄为
/// impl 泛型（形参在前、fresh `T` 在后——`T: Foo<X>` 引用 X，反序 E0401），
/// 实参 = 形参名；trait 级 where 谓词透传 impl where 子句。
/// **assoc type/const 委托**：`@all` 含 const/type 项时生成投影
/// `type Item = <T as Foo<X>>::Item;` / `const N: Ty = <T as Foo<X>>::N;`
/// （不经过 self），解决"带必需关联类型的 trait 做不了"的问题。
/// by-value receiver 方法（`fn consume(self)`）委托语义取决于包装的
/// Deref/move 能力，宏展开期无法区分——维持全放行 + rustc 兜底。
pub(crate) fn expand_blanket(
    args_group: &Group, body: &Group, trait_def: &ItemTrait,
    trait_full_path: &TokenStream,
) -> Result<Vec<TokenTree>, TokenStream> {
    // body 是 Brace 组（`angle_collect` 不进入），其内的 `Cow<'_>` 等扁平
    // `<...>` 未被配对——补一次配对（body 是独立片段，配对安全无副作用）。
    let body_tokens = angle_collect(&body.stream().into_iter().collect::<Vec<_>>())?;
    let wrappers = parse_blanket_wrappers(&body_tokens)?;
    let method_names = parse_names_from_tokens(
        &args_group.stream().into_iter().collect::<Vec<_>>(),
        trait_def,
    )?;
    // fresh 泛型：不与其他名字冲突（`()^N` 元组泛型同款机制）
    let t = fresh_param();

    // 泛型 trait 照抄：形参顺序 = trait 形参在前、fresh T 在后（`T: Foo<X>` 引用 X，反序 E0401）。
    let generics = &trait_def.generics;
    let param_names = crate::analyze::generic_param_names(generics);
    // T 的 bound：`Trait<X>`（含实参）或裸 `Trait`。
    // 实参必须组化为尖括号组（与 trait_part 同款）——组化后解析即正确，不依赖幂等。
    let t_bound = if param_names.is_empty() {
        quote!(#trait_full_path)
    } else {
        let args_group = Group::new(delimiter![<>], quote!(#(#param_names),*));
        quote!(#trait_full_path #args_group)
    };
    // blanket 在 angle_collect 之后运行、产物不再配对，须手动构造组
    // bound 用 trait_full_path（`#[batch_impl_only(#ext::Trait: ...)]` 时是
    // 外部路径，不能用本地 dummy trait 名——裸名在路径前缀场景解析不到）。
    // `<>` 只留名字规范：泛型声明 TypeParam 只取 ident，const/lifetime 原样
    // （`const N: usize` 需完整声明，纯名字 `N` 会 E0747），+ fresh T；
    // 全部约束（trait 形参 inline bound + `T: Trait` + trait where）进 where。
    let impl_names: Vec<TokenStream> = generics
        .params
        .iter()
        .map(|p| match p {
            syn::GenericParam::Type(tp) => {
                let id = &tp.ident;
                quote!(#id)
            }
            syn::GenericParam::Const(cp) => quote!(#cp),
            syn::GenericParam::Lifetime(ld) => quote!(#ld),
        })
        .collect();
    let impl_generics = if impl_names.is_empty() {
        Group::new(delimiter![<>], quote!(#t))
    } else {
        Group::new(delimiter![<>], quote!(#(#impl_names),* , #t))
    };
    // where 基础谓词：`T: Trait`（trait 形参 inline bound 由 codegen 的
    // bound 继承逻辑处理——blanket spec 泛型 X 无 bound，继承自动补 `X: Clone`；
    // 若此处也转移会与继承重复）
    let base_preds: Vec<TokenStream> = vec![quote!(#t : #t_bound)];
    // spec 的 trait 名部分：仅泛型 trait 需要（传实参 `Trait<X>`）；
    // 非泛型 trait 省略（batch_impl 输出时自动补 trait 名——且前缀包装
    // `&^T` 作为目标不能跟在 trait 名后，`Trait &^T` 无法解析）
    let trait_part = if param_names.is_empty() {
        quote!()
    } else {
        let args_group = Group::new(delimiter![<>], quote!(#(#param_names),*));
        quote!(#trait_full_path #args_group)
    };
    // 关联项投影的 `T as Trait<X>` 形态
    let as_trait = if param_names.is_empty() {
        quote!(#t as #trait_full_path)
    } else {
        quote!(#t as #trait_full_path < #(#param_names),*>)
    };

    let mut spec_streams = vec![];
    for wrapper in &wrappers {
        let star = "*".repeat(wrapper.depth + 1);
        let self_ty: TokenStream = format!("{}self", star).parse().unwrap();
        // 包装 where 谓词：`@0` → 目标泛型名；并入 where（零分析并列合并）
        let wrapper_preds = match &wrapper.where_preds {
            Some(preds) => resolve_target_predicates(preds, &t, trait_full_path)?,
            None => vec![],
        };
        // 谓词流整体插入（谓词间逗号已在 token 流内，不能逐 token 逗号连接）
        let mut where_streams = base_preds.clone();
        if let Some(wc) = &generics.where_clause {
            let preds = &wc.predicates;
            where_streams.push(quote!(#preds));
        }
        if !wrapper_preds.is_empty() {
            let wrapper_stream: TokenStream = wrapper_preds.into_iter().collect();
            where_streams.push(wrapper_stream);
        }
        let where_part = if where_streams.is_empty() {
            quote!()
        } else {
            quote!(where { #(#where_streams),* })
        };
        let mut methods = TokenStream::new();
        for name in &method_names {
            let item = get_trait_item(trait_def, name)?;
            match item {
                // 方法：解引用委托
                syn::TraitItem::Fn(f) => {
                    let sig = f.sig.clone();
                    let call_args = collect_call_args(&sig).map_err(|pat| {
                        compile_err!(
                            "batch-impl: #blanket 方法 `{}::{}` 的参数 `{}` 无法委托转发：\
                             仅支持 `self` 与纯标识符模式",
                            trait_def.ident, name, pat
                        )
                    })?;
                    let body = quote! { (#self_ty) . #name ( #(#call_args),* ) };
                    methods.extend(build_from_item(item, &body));
                }
                // 关联类型/常量：投影（不经过 self）
                syn::TraitItem::Type(_) | syn::TraitItem::Const(_) => {
                    let body = quote! { < #as_trait >::#name };
                    methods.extend(build_from_item(item, &body));
                }
                // 理论不可达（trait 定义中只有 fn/const/type）；防御性报错
                _ => {
                    return Err(compile_err!(
                        "batch-impl: #blanket 不支持 trait `{}` 中的 `{}`（未知 item 形态）",
                        trait_def.ident,
                        name
                    ));
                }
            }
        }
        let wrapper_ty = &wrapper.ty;
        spec_streams.push(quote! {
            #impl_generics #trait_part #wrapper_ty ^ #t #where_part { #methods }
        });
    }
    Ok(quote!(#(#spec_streams),*).into_iter().collect())
}

/// 把包装 where 谓词中的位置引用 `@0` 替换为目标泛型名（fresh T）、
/// `@trait` 替换为本地 trait 名。`@N`（N>0）越界报错：blanket 只生成一个目标泛型。
/// `@` 后其他 token 报错——包装 where 只认位置引用与 `@trait`。
fn resolve_target_predicates(
    preds: &[TokenTree], t: &TokenStream, trait_full_path: &TokenStream,
) -> Result<Vec<TokenTree>, TokenStream> {
    let mut out = vec![];
    let mut i = 0;
    while i < preds.len() {
        match &preds[i] {
            TokenTree::Punct(p) if p.as_char() == '@' => match preds.get(i + 1) {
                Some(TokenTree::Literal(lit)) if lit.to_string() == "0" => {
                    out.extend(t.clone());
                    i += 2;
                }
                Some(TokenTree::Literal(lit)) => {
                    return Err(compile_err!(
                        "batch-impl: #blanket 包装 where 中 `@{}` 越界（仅 `@0` 指目标泛型）",
                        lit
                    ));
                }
                Some(TokenTree::Ident(id)) if id == "trait" => {
                    out.extend(trait_full_path.clone());
                    i += 2;
                }
                _ => {
                    return Err(compile_error_str(
                        "batch-impl: #blanket 包装 where 中 `@` 后必须是位置数字（如 `@0`）\
                             或 `@trait`",
                    ));
                }
            },
            _ => {
                out.push(preds[i].clone());
                i += 1;
            }
        }
    }
    Ok(out)
}

/// `#blanket` 的单个包装元素：类型表达式 + 解引用层数 + 可选约束谓词。
struct BlanketWrapper {
    /// 包装类型表达式（不含 `:N` 标注），原样经 `^T` 应用到 fresh 泛型。
    ty: TokenStream,
    /// 委托体解引用层数（`*` 数量 = depth + 1），`:N` 显式标注或默认 1。
    depth: usize,
    /// 包装约束谓词（尾随 `where{...}` 的组内容，`@0` 未解析）。
    /// 并入 impl where 子句（与 trait 泛型 where 谓词并列，零分析合并）。
    where_preds: Option<Vec<TokenTree>>,
}

/// 解析 `#blanket` body 的包装列表（`&,Box^Arc:2,Cow<'_>`，逗号分隔）。
///
/// 元素 = 任意类型 token 流 + 可选尾 `:N` 深度标注（Alone `:` + 数字字面量，
/// 与路径 `::` 的 Joint `:` 不冲突）。元素可为嵌套/预填形态（`&Box`、`Box^Arc`、
/// `Cow<'_>`）。保留三处语法注定错误的报错：`*const`/`*mut`（安全代码无法
/// 解引用裸指针委托）、`self`（无意义）、空元素与非法 `:N`。
fn parse_blanket_wrappers(
    tokens: &[TokenTree],
) -> Result<Vec<BlanketWrapper>, TokenStream> {
    let mut wrappers = vec![];
    let mut current: Vec<TokenTree> = vec![];
    let flush = |mut current: Vec<TokenTree>,
                 wrappers: &mut Vec<BlanketWrapper>|
     -> Result<(), TokenStream> {
        if current.is_empty() {
            return Err(compile_error_str(
                "batch-impl: #blanket 包装列表含空元素（如 `&,Box`）；元素间用 `,` 分隔",
            ));
        }
        // 尾 `where{...}` 约束谓词（元素最后部分，`@0` 指目标泛型；在 `:N` 之后）
        let where_preds = if let Some(TokenTree::Group(g)) = current.last()
            && g.delimiter() == delimiter![{}]
            && let Some(TokenTree::Ident(id)) = current.get(current.len() - 2)
            && id == "where"
        {
            let inner = g.stream().into_iter().collect();
            current.truncate(current.len() - 2);
            Some(inner)
        } else {
            None
        };
        // 尾 `:N` 深度标注（Alone `:`，规则见 doc）
        let mut depth = 1usize;
        let mut ty_end = current.len();
        for i in (0..current.len()).rev() {
            if is_single_colon(&current, i) {
                match &current.get(i + 1) {
                    Some(TokenTree::Literal(lit)) => {
                        depth = lit.to_string().parse().map_err(|_| {
                            compile_err!(
                                "batch-impl: #blanket 的 `:{}` 深度非法（应为正整数，如 `Box^Arc:2`）",
                                lit
                            )
                        })?;
                        if depth == 0 {
                            return Err(compile_error_str(
                                "batch-impl: #blanket 的 `:0` 无意义（解引用层数须 ≥ 1）",
                            ));
                        }
                        ty_end = i;
                    }
                    Some(other) => {
                        return Err(compile_err!(
                            "batch-impl: #blanket 的 `:{}` 后必须是数字（如 `Box^Arc:2`）",
                            other
                        ));
                    }
                    None => {}
                }
                break;
            }
        }
        let ty_tokens = &current[..ty_end];
        match ty_tokens {
            [] => Err(compile_error_str(
                "batch-impl: #blanket 的 `:N` 前缺少包装类型（如 `Box^Arc:2`）",
            )),
            // 内置包装常量：`@Cow` → `Cow<'_>` + 固有约束谓词（deref target = T::Owned，
            // 须 `@0: ToOwned + ?Sized` 与 `@0::Owned: @trait`；@0/@trait 在 resolve 时替换）
            [TokenTree::Punct(at), TokenTree::Ident(name)]
                if at.as_char() == '@' && name == "Cow" =>
            {
                let preds: Vec<TokenTree> =
                    quote!(@0: ToOwned + ?Sized, @0::Owned: @trait)
                        .into_iter()
                        .collect();
                // quote 不配对尖括号——`Cow<'_>` 须手动构造 <> 组
                //（blanket 产物不再过 angle_collect，扁平 `<` 会残留）
                let args = Group::new(delimiter![<>], quote!('_));
                wrappers.push(BlanketWrapper {
                    ty: quote!(Cow #args),
                    depth,
                    where_preds: Some(preds),
                });
                Ok(())
            }
            [TokenTree::Punct(a), TokenTree::Ident(n)]
                if a.as_char() == '*' && (n == "const" || n == "mut") =>
            {
                Err(compile_error_str(
                    "batch-impl: #blanket 不支持 `*const`/`*mut` 包装（解引用 unsafe，\
                     无法委托）；请手写 #delegate",
                ))
            }
            [TokenTree::Ident(id)] if id == "self" => Err(compile_error_str(
                "batch-impl: #blanket 不支持 `self` 包装（委托无意义）；请手写 #delegate",
            )),
            _ => {
                let ty = ty_tokens.iter().cloned().collect();
                wrappers.push(BlanketWrapper { ty, depth, where_preds });
                Ok(())
            }
        }
    };
    for tt in tokens {
        if let TokenTree::Punct(p) = tt
            && p.as_char() == ','
        {
            flush(current, &mut wrappers)?;
            current = vec![];
        } else {
            current.push(tt.clone());
        }
    }
    flush(current, &mut wrappers)?;
    Ok(wrappers)
}
