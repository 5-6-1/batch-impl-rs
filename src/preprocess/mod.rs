//! 预处理层：指令展开、裸 where 改写、尖括号组配对。

// ============================================================
// 分隔符拼写宏
// ============================================================

/// 分隔符拼写宏：统一 `Delimiter::*` 字面量为源码分隔符拼写
/// （调用统一用 `[]`）——`delimiter![{}]` / `delimiter![[]]` /
/// `delimiter![()]` 与源码一一对应。
///
/// proc-macro2 的 `Delimiter` 无"尖括号"变体，`<>` 必须借用 `Delimiter::None`
/// 承载——而 `None` 本身也是真实"透明组"的拼写。为避免两义，宏用两种拼写
/// 区分：
/// - `delimiter![<>]`：**尖括号组**载体（`angle_collect` 配对产物）；
/// - `delimiter![none]`：**真实透明组**（宏变量 `$var:ty` 展开产物，
///   内容即 DSL token，需扁平化）。
///
/// 二者展开值相同（`Delimiter::None`），不可在同一条 `match` 中作两个臂
/// （会报 unreachable pattern）；实际用法分布在互斥的上下文，无冲突。
macro_rules! delimiter {
    ({}) => {
        ::proc_macro2::Delimiter::Brace
    };
    ([]) => {
        ::proc_macro2::Delimiter::Bracket
    };
    (()) => {
        ::proc_macro2::Delimiter::Parenthesis
    };
    (<>) => {
        ::proc_macro2::Delimiter::None
    };
    (none) => {
        ::proc_macro2::Delimiter::None
    };
}

pub(crate) mod angle;
pub(crate) mod preprocess_helpers;
pub(crate) mod where_process;

pub(crate) use angle::*;
pub(crate) use preprocess_helpers::*;
pub(crate) use where_process::*;

use proc_macro2::{Group, Ident, TokenStream, TokenTree};
use quote::quote;
use syn::ItemTrait;

use crate::ast::fresh_param;
use crate::diagnostic::compile_error_str;
use crate::scan::Cursor;

// ============================================================
// 指令预处理
// ============================================================

/// 指令预处理入口：扫描 token 流，展开 `#` 指令。
///
/// 仅 `#[batch_impl]` / `#[batch_impl_only]` 支持（需要 trait 定义读取方法签名）。
/// `batch_trait!` 不调用此函数（无 trait 定义可用）。
///
/// ## 指令语法
///
/// | 指令 | 语法 | 效果 |
/// |------|------|------|
/// | 单 item | `#name{body}` | `{fn method(签名) { body }}` 或 `{const NAME: Type = body;}` 或 `{type Name = body;}` |
/// | 填充 | `#fill(args){body}` | `{fn m1(sig){body} fn m2(sig){body} ...}` |
/// | 委托 | `#delegate(args){target}` | `{fn m1(sig){(target).m1(args)} ...}` |
/// | 覆盖 | `#blanket(args){包装列表}` | 多段完整 spec（见 [`expand_blanket`]） |
///
/// 展开产物：既有指令恰为一个 `{...}` 组（可附着到类型或独立成 spec）；
/// `#blanket` 产出多段 spec，只能独立（自含泛型/目标/委托，见
/// architecture.md「语法域隔离」的附着语义说明）。
///
/// `args` 中出现 `#all` 表示 trait 的所有 item（fn + const + type），
/// `#all_methods` 仅 Fn 方法，`#all_constants` 仅 const，`#all_types` 仅 type。
///
/// ## 递归规则
///
/// 只递归展开 `[...]`（Bracket）Group 内容；`(...)` 和 `{...}` 不递归，
/// 避免误入指令的参数或 body。
pub(crate) fn expand_tokens(
    cursor: &mut Cursor, trait_def: &ItemTrait, trait_full_path: &TokenStream,
) -> Result<Vec<TokenTree>, TokenStream> {
    let mut result = vec![];
    while !cursor.at_end() {
        if cursor.is_punct('#')
            && let Some(TokenTree::Ident(name)) = cursor.peek_at(1)
        {
            // 指令展开为 0..n 个 token：既有指令（#name/#fill/#delegate/开放扩展）
            // 产物恰为一个 `{...}` 组；blanket 等新指令产出完整 spec 多 token。
            result.extend(expand_directive(
                name,
                cursor,
                trait_def,
                trait_full_path,
            )?);
            continue;
        }
        // 当前 token 一定存在（循环条件保证了非 at_end）
        let Some(tt) = cursor.peek() else {
            // 逻辑上不可达；防御性 break 以兜底
            break;
        };
        // 只递归展开 [...] 内容（`(...)` 和 `{...}` 不递归）；
        // `ident![...]` 宏调用体与 `#[...]` 属性是透传的（内容任意 Rust，
        // 不得展开其中的指令——与 angle_collect 的 Bracket 守卫对齐）
        if let TokenTree::Group(g) = tt
            && g.delimiter() == delimiter![[]]
            && !cursor.prev_bracket_passthrough()
        {
            let inner = expand_tokens(
                &mut Cursor::new(&g.stream().into_iter().collect::<Vec<_>>()),
                trait_def,
                trait_full_path,
            )?;
            let new_group = Group::new(g.delimiter(), inner.into_iter().collect());
            result.push(new_group.into());
            cursor.bump();
        } else {
            result.push(tt.clone());
            cursor.bump();
        }
    }
    Ok(result)
}

/// 分派指令：根据 `#` 后的名称和括号结构分派到对应的展开函数。
///
/// 返回 `Vec<TokenTree>`：既有指令（`#name`/`#fill`/`#delegate`/开放扩展）
/// 产物恰为一个 `{...}` 组；`#blanket` 产出完整 spec 多 token（见
/// [`expand_blanket`]）。调用方以 `extend` 并入结果流。
fn expand_directive(
    name: &Ident, cursor: &mut Cursor, trait_def: &ItemTrait,
    trait_full_path: &TokenStream,
) -> Result<Vec<TokenTree>, TokenStream> {
    if let Some(TokenTree::Group(args)) = cursor.peek_at(2) {
        match args.delimiter() {
            delimiter![{}] => {
                // `#name{body}` — item 名紧跟 `{body}`（fn / const / type 通用）
                cursor.bump(); // #
                cursor.bump(); // method_name
                cursor.bump(); // {body}
                expand_single(name, args, trait_def).map(|tt| vec![tt])
            }
            _ => {
                // `#cmd(args){body}` — 名称 + 括号参数 + {body}
                let body_tt = cursor.peek_at(3);
                let Some(TokenTree::Group(body)) = body_tt else {
                    return Err(compile_error_str(&format!(
                        "`#{}` 后期望 `(args)` + `{{body}}` 或直接 `{{body}}`",
                        name
                    )));
                };
                if body.delimiter() != delimiter![{}] {
                    return Err(compile_error_str(&format!(
                        "`#{}` 后期望 `(args)` + `{{body}}` 或直接 `{{body}}`",
                        name
                    )));
                }
                cursor.bump(); // #
                cursor.bump(); // name
                cursor.bump(); // (args)
                cursor.bump(); // {body}
                match name.to_string().as_str() {
                    "fill" => expand_fill(args, body, trait_def).map(|tt| vec![tt]),
                    "delegate" => {
                        expand_delegate(args, body, trait_def).map(|tt| vec![tt])
                    }
                    "blanket" => {
                        expand_blanket(args, body, trait_def, trait_full_path)
                    }
                    // 开放扩展：`#name(args){body}` → `{ name!{(args){body} trait_def} }`
                    // 一个函数式宏调用，位于 impl body（附着用法）或顶层（独立用法）。
                    // 与 `#fill`/`#delegate` 同源：把"读 trait → 生成 fn 定义"的实现
                    // 交给用户的同名宏——它解析 args / body / trait 并生成 impl 项。
                    _ => {
                        let inner = quote! {
                            #name ! { #args #body #trait_def }
                        };
                        Ok(vec![Group::new(delimiter![{}], inner).into()])
                    }
                }
            }
        }
    } else {
        Err(compile_error_str(&format!(
            "`#{}` 后期望括号参数 `(args)` 或代码块 `{{body}}`",
            name
        )))
    }
}

/// `#name{body}` → `{fn method(签名) { body }}` 或 `{const NAME: Type = body;}` 或 `{type Name = body;}`
///
/// 根据 `name` 在 trait 定义中查找对应的 item，由 `build_from_item` 按 item 类型自动输出。
fn expand_single(
    method_name: &Ident, body: &Group, trait_def: &ItemTrait,
) -> Result<TokenTree, TokenStream> {
    let item = get_trait_item(trait_def, method_name)?;
    Ok(Group::new(delimiter![{}], build_from_item(item, &body.stream())).into())
}

/// 多 item 指令展开的公共骨架：解析方法名列表 → 逐 item 构造实现 → 打包为 `{...}` 组。
/// `build` 按 item 构造实现体（可报错，如 `#delegate` 的非 fn 项/解构参数）。
fn expand_many(
    args_group: &Group, trait_def: &ItemTrait,
    build: impl Fn(&Ident, &syn::TraitItem) -> Result<TokenStream, TokenStream>,
) -> Result<TokenTree, TokenStream> {
    let method_names = parse_names_from_tokens(
        &args_group.stream().into_iter().collect::<Vec<_>>(),
        trait_def,
    )?;
    let mut methods = TokenStream::new();
    for name in &method_names {
        let item = get_trait_item(trait_def, name)?;
        methods.extend(build(name, item)?);
    }
    Ok(Group::new(delimiter![{}], methods).into())
}

/// `#fill(args){body}` → `{fn m1(sig){body} fn m2(sig){body} ...}`
///
/// `args` 为逗号分隔的 item 名列表，或 `#all`（表示所有 item）。
/// 支持 fn、const、type 三种 item 类型。
/// 为每个 item 从 trait 定义读取签名/类型，body 作为实现体。
fn expand_fill(
    args_group: &Group, body: &Group, trait_def: &ItemTrait,
) -> Result<TokenTree, TokenStream> {
    let body_stream = body.stream();
    expand_many(args_group, trait_def, |_name, item| {
        Ok(build_from_item(item, &body_stream))
    })
}

/// `#delegate(args){target}` → `{fn m1(sig){(target).m1(params)} ...}`
///
/// 为每个方法生成委托调用：跳过 `self` 参数，将其余参数原样转发。
fn expand_delegate(
    args_group: &Group, target: &Group, trait_def: &ItemTrait,
) -> Result<TokenTree, TokenStream> {
    let target_stream = target.stream();
    expand_many(args_group, trait_def, |name, item| {
        let syn::TraitItem::Fn(f) = item else {
            return Err(compile_error_str(&format!(
                "batch-impl: #delegate 只能用于方法，trait `{}` 中的 `{}` 不是方法",
                trait_def.ident, name
            )));
        };
        let sig = f.sig.clone();
        let call_args = collect_call_args(&sig).map_err(|pat| {
            compile_error_str(&format!(
                "batch-impl: #delegate 方法 `{}::{}` 的参数 `{}` 无法委托转发：\
                 仅支持 `self` 与纯标识符模式",
                trait_def.ident, name, pat
            ))
        })?;
        let body = quote! { (#target_stream) . #name ( #(#call_args),* ) };
        Ok(build_from_item(item, &body))
    })
}

/// `#blanket(#all){&,Box,Rc}` — 覆盖式委托：为每个包装类型生成一段完整 spec。
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
/// **assoc type/const 委托**：`#all` 含 const/type 项时生成投影
/// `type Item = <T as Foo<X>>::Item;` / `const N: Ty = <T as Foo<X>>::N;`
/// （不经过 self），解决"带必需关联类型的 trait 做不了"的问题。
/// by-value receiver 方法（`fn consume(self)`）委托语义取决于包装的
/// Deref/move 能力，宏展开期无法区分——维持全放行 + rustc 兜底。
///
/// 产物为多段完整 spec（`<T: Trait> 包装^T { 方法体 }`，逗号分隔），只能独立
/// 成 spec——与 `expand_directive` 的 `Vec<TokenTree>` 返回契约配套。
fn expand_blanket(
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

    // 泛型 trait 照抄（与 0.5.5 `A<>` 同机制，但 blanket 整段自生成、无合并）：
    // 形参顺序 = trait 形参在前、fresh T 在后（`T: Foo<X>` 引用 X，反序 E0401）。
    let generics = &trait_def.generics;
    let param_names: Vec<TokenStream> = generics
        .params
        .iter()
        .map(|p| match p {
            syn::GenericParam::Lifetime(ld) => quote!(#ld),
            syn::GenericParam::Type(tp) => {
                // 注意：quote 插值不支持字段访问（`#tp.ident` 会把 `.ident`
                // 当字面量），先取引用再插值
                let id = &tp.ident;
                quote!(#id)
            }
            syn::GenericParam::Const(cp) => {
                let id = &cp.ident;
                quote!(#id)
            }
        })
        .collect();
    // T 的 bound：`Trait<X>`（含实参）或裸 `Trait`。
    // 实参必须组化为尖括号组（与 trait_part 同款）：扁平 `<A, B>` 会被
    // parse_angle_bracket_contents 的 depth-0 逗号切分错误切断
    // （`T: Two<A, B>` → `T: Two<A` / `B>`），此前靠渲染幂等侥幸正确——
    // 组化后解析即正确，不依赖幂等。
    let t_bound = if param_names.is_empty() {
        quote!(#trait_full_path)
    } else {
        let args_group = Group::new(delimiter![<>], quote!(#(#param_names),*));
        quote!(#trait_full_path #args_group)
    };
    // 泛型声明须为尖括号组（`angle_collect` 配对产物形态）：blanket 在
    // angle_collect 之后运行，产出的 `<...>` 不会再次配对，须手动构造组。
    // bound 用 trait_full_path（`#[batch_impl_only(#ext::Trait: ...)]` 时是
    // 外部路径，不能用本地 dummy trait 名——裸名在路径前缀场景解析不到）。
    let impl_generics_inner = if generics.params.is_empty() {
        quote!(#t : #t_bound)
    } else {
        let ps = &generics.params;
        quote!(#ps , #t : #t_bound)
    };
    let impl_generics = Group::new(delimiter![<>], impl_generics_inner);
    // spec 的 trait 名部分：仅泛型 trait 需要（传实参 `Trait<X>`）；
    // 非泛型 trait 省略（batch_impl 输出时自动补 trait 名——且前缀包装
    // `&^T` 作为目标不能跟在 trait 名后，`Trait &^T` 无法解析）
    let trait_part = if param_names.is_empty() {
        quote!()
    } else {
        let args_group = Group::new(delimiter![<>], quote!(#(#param_names),*));
        quote!(#trait_full_path #args_group)
    };
    // trait 级 where 谓词透传 impl where（全部谓词——单一形参谓词未并入
    // blanket 的 impl 泛型内联 bound，须经 where 生效）
    let where_part = match &generics.where_clause {
        Some(wc) => {
            let preds = &wc.predicates;
            quote!(where { #preds })
        }
        None => quote!(),
    };
    // 关联项投影的 `T as Trait<X>` 形态
    let as_trait = if param_names.is_empty() {
        quote!(#t as #trait_full_path)
    } else {
        quote!(#t as #trait_full_path < #(#param_names),*>)
    };

    let mut spec_streams = vec![];
    for wrapper in &wrappers {
        // 委托体：`*` 数量 = depth + 1
        let star = "*".repeat(wrapper.depth + 1);
        let self_ty: TokenStream = format!("{}self", star).parse().unwrap();
        let mut methods = TokenStream::new();
        for name in &method_names {
            let item = get_trait_item(trait_def, name)?;
            match item {
                // 方法：解引用委托 `(**self).m(args)`
                syn::TraitItem::Fn(f) => {
                    let sig = f.sig.clone();
                    let call_args = collect_call_args(&sig).map_err(|pat| {
                        compile_error_str(&format!(
                            "batch-impl: #blanket 方法 `{}::{}` 的参数 `{}` 无法委托转发：\
                             仅支持 `self` 与纯标识符模式",
                            trait_def.ident, name, pat
                        ))
                    })?;
                    let body = quote! { (#self_ty) . #name ( #(#call_args),* ) };
                    methods.extend(build_from_item(item, &body));
                }
                // 关联类型/常量：投影 `T as Trait<X>::Item`（不经过 self）
                syn::TraitItem::Type(_) | syn::TraitItem::Const(_) => {
                    let body = quote! { < #as_trait >::#name };
                    methods.extend(build_from_item(item, &body));
                }
                // 理论不可达（trait 定义中只有 fn/const/type）；防御性报错
                _ => {
                    return Err(compile_error_str(&format!(
                        "batch-impl: #blanket 不支持 trait `{}` 中的 `{}`（未知 item 形态）",
                        trait_def.ident, name
                    )));
                }
            }
        }
        // 目标类型 = 包装表达式 + `^T`：`Box^Arc:2` → `Box^Arc^T`（右结合
        // `Box<Arc<T>>`）、`Cow<'_>` → `Cow<'_>^T`（预填追加 `Cow<'_, T>`）
        let wrapper_ty = &wrapper.ty;
        spec_streams.push(quote! {
            #impl_generics #trait_part #wrapper_ty ^ #t #where_part { #methods }
        });
    }
    Ok(quote!(#(#spec_streams),*).into_iter().collect())
}

/// `#blanket` 的单个包装元素：类型表达式 + 解引用层数（`:N` 标注或默认 1）。
struct BlanketWrapper {
    /// 包装类型表达式（不含 `:N` 标注），原样经 `^T` 应用到 fresh 泛型。
    ty: TokenStream,
    /// 委托体解引用层数（`*` 数量 = depth + 1），`:N` 显式标注或默认 1。
    depth: usize,
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
    let flush = |current: Vec<TokenTree>,
                 wrappers: &mut Vec<BlanketWrapper>|
     -> Result<(), TokenStream> {
        if current.is_empty() {
            return Err(compile_error_str(
                "batch-impl: #blanket 包装列表含空元素（如 `&,Box`）；元素间用 `,` 分隔",
            ));
        }
        // 尾 `:N` 深度标注：最后一个 Alone `:`（`is_single_colon`，与 `::` 的
        // Joint `:` 不冲突）后接数字字面量；无标注默认 1（宏不猜包装内部
        // Deref 层数，嵌套须显式 `:N`）。
        let mut depth = 1usize;
        let mut ty_end = current.len();
        for i in (0..current.len()).rev() {
            if crate::scan::is_single_colon(&current, i) {
                match &current.get(i + 1) {
                    Some(TokenTree::Literal(lit)) => {
                        depth = lit.to_string().parse().map_err(|_| {
                            compile_error_str(&format!(
                                "batch-impl: #blanket 的 `:{}` 深度非法（应为正整数，如 `Box^Arc:2`）",
                                lit
                            ))
                        })?;
                        if depth == 0 {
                            return Err(compile_error_str(
                                "batch-impl: #blanket 的 `:0` 无意义（解引用层数须 ≥ 1）",
                            ));
                        }
                        ty_end = i;
                    }
                    Some(other) => {
                        return Err(compile_error_str(&format!(
                            "batch-impl: #blanket 的 `:{}` 后必须是数字（如 `Box^Arc:2`）",
                            other
                        )));
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
                wrappers.push(BlanketWrapper { ty, depth });
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
