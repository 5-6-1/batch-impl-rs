#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))]
// 库不使用任何 unsafe；缺失文档按错误拒绝（仅作用于 pub 项，内部 pub(crate) 不受限）。
#![forbid(unsafe_code)]
#![deny(missing_docs)]
// MSVC 链接器输出"正在创建库…和对象…"到 stdout，被 rustc 当 linker_messages 告警，
// 属于无害的 Windows 链接产物提示，全局抑制。
#![allow(linker_messages)]
#[cfg(test)]
mod fuzz;
use proc_macro2::{TokenStream, TokenTree};
use quote::quote;
use syn::{ItemTrait, parse_macro_input};

mod apply;
mod apply_tuple;
mod batch_trait_entry;
mod codegen;
mod diagnostic;
mod generic;
mod parse;
mod parse_atom;
mod path_prefix;
mod preprocess;
mod preprocess_helpers;
mod scan;
mod types;
mod types_render;
mod where_process;

use batch_trait_entry::parse_batch_trait_entry;

use diagnostic::compile_error_str;
use preprocess_helpers::{build_from_item, get_trait_item, parse_names_from_tokens};
use scan::{Cursor, is_arrow};
use types::{Op, reset_fresh_counter};
use where_process::where_process;

/// 为 trait 批量生成 `impl` 块的属性宏。
///
/// 在 trait 定义上标注 `#[batch_impl(...)]`，宏参数中的每个 impl-spec 都会
/// 为该 trait 生成一个对应的 `impl` 块。
///
/// ## 语法
///
/// ```text
/// #[batch_impl( impl-spec [, impl-spec]* [{ body }]? )]
/// ```
///
/// impl-spec 由三部分组成（均可省略后半部分）：
/// - `<impl-泛型>` — `impl` 块的泛型参数
/// - `Trait名<trait-泛型>` — trait 的泛型参数与关联类型绑定
/// - 目标类型 — 用 `[]` 包裹表示并列，用 `^`/`-` 表示泛型应用
///
/// ## 示例
///
/// ```
/// # use batch_impl::batch_impl;
/// #[batch_impl(usize, isize)]
/// trait Numeric {}
///
/// #[batch_impl(<T> Vec<T>)]
/// trait Collection {}
///
/// #[batch_impl(<T> FromValue<T> [i32 { fn wrap(_: T) -> Self { 0 }}, u32 #wrap{0}] )]
/// trait FromValue<T> { fn wrap(val: T) -> Self; }
///
/// // #name{body} 也支持 const 和 type 项
/// #[batch_impl(usize #MY_CONST{42})]
/// trait HasConst { const MY_CONST: usize; }
///
/// ```
#[proc_macro_attribute]
pub fn batch_impl(
    attr: proc_macro::TokenStream, item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let trait_item = parse_macro_input!(item as ItemTrait);
    expand_attr_macro(attr, trait_item, true).unwrap_or_else(Into::into)
}

/// 与 `#[batch_impl]` 相同，但丢弃被标注的 trait 定义，只输出 `impl` 块。
///
/// 用于 trait 已在别处定义、只需批量生成 impl 的场景。被标注的 trait 仅作为
/// 指令系统的"签名真相源"：`#name`/`#fill`/`#delegate` 从它读取 item 签名，
/// 开放扩展 `#name(args){body}` 把（方法名列表, body, 整个 trait）一起交给
/// 用户的同名函数式宏（见 README「指令系统」）。语法与 `#[batch_impl]` 完全一致。
///
/// ## 示例
///
/// ```
/// # use batch_impl::batch_impl_only;
/// trait Greet { fn hello(&self) -> &str; }
///
/// #[batch_impl_only(usize #hello{"hi"})]
/// trait Greet { fn hello(&self) -> &str; } // 此 trait 定义被丢弃，不影响已有的定义
/// // 这样写而不用batch_trait是为了使用指令系统，建议按trait定义处按原样写
/// ```
#[proc_macro_attribute]
pub fn batch_impl_only(
    attr: proc_macro::TokenStream, item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let trait_item = parse_macro_input!(item as ItemTrait);
    expand_attr_macro(attr, trait_item, false).unwrap_or_else(Into::into)
}

/// 两个属性宏的共享实现（错误经 `compile_error!` token 流返回）
fn expand_attr_macro(
    attr: proc_macro::TokenStream, trait_item: ItemTrait, include_trait: bool,
) -> Result<proc_macro::TokenStream, TokenStream> {
    reset_fresh_counter();
    let trait_name = trait_item.ident.clone();
    let attr_vec = TokenStream::from(attr).into_iter().collect::<Vec<_>>();

    // `#[batch_impl_only]` 专属：attr 起首若是 `# Path: ` 形式
    // （`#` + `Ident (:: Ident)*` + `:`），则把该路径作为外部 trait 路径，
    // 余下 attr 作为 DSL spec。`#[batch_impl]` 不支持此前缀
    // （它输出本地 trait 定义，路径前缀无意义）。
    let (trait_full_path, trait_last_ident, rest_tokens) = if !include_trait {
        match path_prefix::try_parse_path_prefix(&attr_vec) {
            Some((path, last_ident, rest)) => {
                // 路径前缀的 last ident 必须与本地 dummy trait 名一致，
                // 否则后续 DSL 中的 `Trait<T>` 匹配会失败。
                match last_ident {
                    Some(id) if id == trait_name => {
                        let path_ts = path.into_iter().collect();
                        // 此处借用本地 trait_name 作为匹配标识
                        // （已校验与路径末段同名）。
                        (path_ts, trait_name.clone(), rest)
                    }
                    Some(id) => {
                        let msg = format!(
                            "batch-impl: 路径前缀 `#...{}` \
                                 的末尾标识符与 trait 名 `{}` \
                                 不一致；二者必须相同",
                            id, trait_name,
                        );
                        return Err(compile_error_str(&msg));
                    }
                    None => {
                        let msg = "batch-impl: 路径前缀 `#` 后 \
                                 期望至少一个标识符作为 trait 路径";
                        return Err(compile_error_str(msg));
                    }
                }
            }
            None => (quote![#trait_name], trait_name.clone(), attr_vec.clone()),
        }
    } else {
        (quote![#trait_name], trait_name.clone(), attr_vec.clone())
    };

    let mut cursor = Cursor::new(&rest_tokens);
    let expanded = preprocess::expand_tokens(&mut cursor, &trait_item)?;
    // 裸 `where 谓词 {body}` 新语法 → 统一改写为旧式 `where{谓词}`
    // （指令预处理之后、DSL 解析之前；三个接口共用）
    let expanded = where_process(&mut Cursor::new(&expanded))?;
    let is_unsafe = trait_item.unsafety.is_some();
    let trait_bounds = extract_trait_bounds(&trait_item);
    // `A<>`：trait 泛型照抄（实参与 bound 全部来自 trait 定义）。
    // 在指令预处理与 where 改写之后、DSL 解析之前展开为
    // `<'a, T: bounds, const N> A<'a, T, N>`——展开产物与手写完全等价。
    let expanded = expand_empty_trait_generics(&expanded, &trait_item)?;
    cursor = Cursor::new(&expanded);
    let start_trait = if include_trait { trait_item.into() } else { None };
    let impls = parse_batch_trait_entry(
        &mut cursor,
        Op::Comma,
        &trait_full_path,
        &trait_last_ident,
        is_unsafe,
        start_trait,
        &trait_bounds,
    );
    Ok(impls.into())
}

/// trait 形参：名字 + 内联 bound + bound 引用的形参名（token 级保守检测）。
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
/// 隐含 `T: A`）宏无法推理）。trait 级 where 子句不继承（第一版范围）。
#[derive(Default)]
pub(crate) struct TraitBounds {
    pub(crate) params: Vec<TraitParam>,
}

fn extract_trait_bounds(trait_item: &ItemTrait) -> TraitBounds {
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
    TraitBounds { params }
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

/// `A<>` 预处理：扫描 token 流中深度 0 的 `Ident < >`（空 trait 实参），
/// 展开为 `<'a, T: bounds, const N> Ident<'a, T, N>`（impl 泛型段 + 实参段）。
///
/// - 只处理深度 0 的 `Ident<>`（`B<A<>>` 嵌套不展开）；
/// - trait 无泛型参数时透传（`A<>` 由 DSL 解析为空实参，渲染 `A`）；
/// - `->` 箭头的 `>` 不计深度（复用 scan 的箭头守卫）；
/// - 仅 `#[batch_impl]` / `#[batch_impl_only]` 可用（需要 trait 定义渲染形参）；
///   `batch_trait!` 无 trait 定义，`A<>` 原样透传。
fn expand_empty_trait_generics(
    tokens: &[TokenTree], trait_def: &ItemTrait,
) -> Result<Vec<TokenTree>, TokenStream> {
    if trait_def.generics.params.is_empty() {
        return Ok(tokens.to_vec());
    }
    // 预渲染展开段：impl 泛型（含尖括号）+ 实参名列表
    let generics = &trait_def.generics;
    let impl_gen = quote!(#generics);
    let mut arg_names: Vec<TokenStream> = vec![];
    for p in &trait_def.generics.params {
        match p {
            syn::GenericParam::Lifetime(ld) => arg_names.push(quote!(#ld)),
            syn::GenericParam::Type(tp) => {
                let id = &tp.ident;
                arg_names.push(quote!(#id));
            }
            syn::GenericParam::Const(cp) => {
                let id = &cp.ident;
                arg_names.push(quote!(#id));
            }
        }
    }
    let mut out = vec![];
    let mut depth = 0usize;
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            TokenTree::Punct(p) if p.as_char() == '<' => {
                depth += 1;
                out.push(tokens[i].clone());
                i += 1;
            }
            TokenTree::Punct(p) if p.as_char() == '>' => {
                if !is_arrow(tokens, i) {
                    depth = depth.saturating_sub(1);
                }
                out.push(tokens[i].clone());
                i += 1;
            }
            TokenTree::Ident(id)
                if depth == 0
                    && matches!(tokens.get(i + 1), Some(TokenTree::Punct(p)) if p.as_char() == '<')
                    && matches!(tokens.get(i + 2), Some(TokenTree::Punct(p)) if p.as_char() == '>') =>
            {
                // `A<>` → `<'a, T: bounds, const N> A<'a, T, N>`
                let name = quote!(#id);
                out.extend(impl_gen.clone());
                out.extend(quote!(#name < #(#arg_names),* >));
                i += 3;
            }
            _ => {
                out.push(tokens[i].clone());
                i += 1;
            }
        }
    }
    Ok(out)
}

/// 对已声明的 trait 批量生成 `impl` 块的函数式宏。
///
/// 语法：`unsafe? Trait路径: impl-specs;`，以 `;` 分隔多个 trait 段。
/// 每段的 `:` 之后是 DSL 表达式，与 `#[batch_impl]` 接受相同的语法。
///
/// ## 示例
///
/// ```
/// # use batch_impl::batch_trait;
/// trait A {}
/// trait B<T> {}
/// unsafe trait UnsafeTrait{}
///
/// batch_trait!(
///     A: usize, isize;
///     B: <T> B<T> Vec<T>;
///     unsafe UnsafeTrait: usize
/// );
/// ```
///
/// 路径 trait（如 `foo::C`）同样支持，见 tests/regression.rs。
#[proc_macro]
pub fn batch_trait(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    expand_batch_trait(input).unwrap_or_else(Into::into)
}

/// `batch_trait!` 的实际展开（错误经 `compile_error!` token 流返回）
fn expand_batch_trait(
    input: proc_macro::TokenStream,
) -> Result<proc_macro::TokenStream, TokenStream> {
    reset_fresh_counter();
    let tokens = TokenStream::from(input).into_iter().collect::<Vec<_>>();
    let tokens = where_process(&mut Cursor::new(&tokens))?;
    let mut cursor = Cursor::new(&tokens);
    let mut result = quote![];
    loop {
        // 跳过前导 `;`（允许连续多个分号，尾随分号）
        while cursor.is_punct(';') {
            cursor.bump();
        }
        if cursor.at_end() {
            break;
        }

        // `unsafe` 前缀：标记该段所有 impl 为 unsafe impl
        let is_unsafe = if matches!(cursor.peek(), Some(TokenTree::Ident(id)) if *id == "unsafe")
        {
            cursor.bump();
            true
        } else {
            false
        };

        // 收集 trait 路径（遇到 `<>` 深度为 0 的 `:` 停止；`::` 路径分隔符一并收集）
        let path_start = cursor.pos();
        let mut depth = 0i32;
        while let Some(token) = cursor.peek() {
            match token {
                TokenTree::Punct(p) if p.as_char() == '<' => {
                    depth += 1;
                    cursor.bump();
                }
                TokenTree::Punct(p) if p.as_char() == '>' => {
                    depth -= 1;
                    cursor.bump();
                }
                TokenTree::Punct(p) if p.as_char() == ':' && depth == 0 => {
                    if cursor.is_single_colon() {
                        break;
                    } else {
                        cursor.bump();
                        cursor.bump();
                    }
                }
                _ => cursor.bump(),
            }
        }
        let trait_path = cursor.slice_since(path_start);
        if trait_path.is_empty() {
            result.extend(compile_error_str("batch_trait! 中期望 trait 名称"));
            break;
        }
        // trait 完整路径：原样收集 trait_path 的 token 流即可
        let trait_full_path = trait_path.iter().cloned().collect();
        // 取路径中的最后一个标识符作为 `trait_name` 匹配用
        let trait_last_ident =
            match trait_path
                .iter()
                .filter_map(|tt| {
                    if let TokenTree::Ident(id) = tt { id.into() } else { None }
                })
                .next_back()
            {
                Some(ident) => ident,
                None => {
                    result.extend(compile_error_str(
                        "batch_trait! 中期望标识符作为 trait 名称",
                    ));
                    break;
                }
            };
        if !cursor.is_punct(':') {
            result.extend(compile_error_str(
                "batch_trait! 中期望 ':' 分隔 trait 名称和 impl-specs",
            ));
            break;
        }
        cursor.bump();
        let impl_code = parse_batch_trait_entry(
            &mut cursor,
            Op::Semi,
            &trait_full_path,
            trait_last_ident,
            is_unsafe,
            None,
            // batch_trait! 无 trait 定义，无法继承泛型 bound
            &Default::default(),
        );
        result.extend(impl_code);
    }
    Ok(result.into())
}

/// 测试用开放扩展宏（函数式）：`name!{(方法名列表){body} trait T {...}}`。
///
/// 从宏输入解析方法名列表、body 与 trait 定义，为每个方法生成
/// `fn 签名 { body }`（沿用 trait 签名）——等价于把 `#fill` 的实现交给用户。
///
/// 用于验证开放指令扩展：`#name(args){body}` 展开为 `{name!{(args){body} trait ...}}`，
/// 宏调用落在 impl body 中，由用户宏根据 trait 展开为需要的 fn 定义
/// （见 `tests/dsl.rs` 第 28 节）。
///
/// 设计要点：这里必须是**函数式宏调用** `name!{...}`，不能是 `#[name[...]] trait ...`
/// 属性——trait 不是 impl 块内的合法项（`#[attr] trait` 无法出现在 impl 中），
/// 而函数式宏在 impl body 位置会被 rustc 展开成关联项。
#[doc(hidden)]
#[proc_macro]
pub fn batch_preprocess_test(
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let tokens = TokenStream::from(input).into_iter().collect::<Vec<_>>();
    // 形如：`(add, inc) {*self+1} trait AddInc {...}`
    let Some(TokenTree::Group(names_group)) = tokens.first() else {
        return compile_error_str(
            "batch-impl: batch_preprocess_test 期望 `(方法名列表){body} trait ...`",
        )
        .into();
    };
    if names_group.delimiter() != proc_macro2::Delimiter::Parenthesis {
        return compile_error_str(
            "batch-impl: batch_preprocess_test 期望 `(方法名列表){body} trait ...`",
        )
        .into();
    }
    let Some(TokenTree::Group(body_group)) = tokens.get(1) else {
        return compile_error_str(
            "batch-impl: batch_preprocess_test 期望 `(方法名列表){body} trait ...`",
        )
        .into();
    };
    if body_group.delimiter() != proc_macro2::Delimiter::Brace {
        return compile_error_str(
            "batch-impl: batch_preprocess_test 期望 `(方法名列表){body} trait ...`",
        )
        .into();
    }
    let trait_ts = tokens[2..].iter().cloned().collect();
    let trait_item = match syn::parse2(trait_ts) {
        Ok(t) => t,
        Err(_) => {
            return compile_error_str(
                "batch-impl: batch_preprocess_test 无法解析 trait 定义",
            )
            .into();
        }
    };
    let names = match parse_names_from_tokens(
        &names_group.stream().into_iter().collect::<Vec<_>>(),
        &trait_item,
    ) {
        Ok(names) => names,
        Err(e) => return e.into(),
    };
    let body = body_group.stream();
    let mut methods = TokenStream::new();
    for name in &names {
        let item = match get_trait_item(&trait_item, name) {
            Ok(item) => item,
            Err(e) => return e.into(),
        };
        methods.extend(build_from_item(item, &body));
    }
    methods.into()
}
