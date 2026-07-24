use proc_macro::TokenStream;
use proc_macro2::{Ident, TokenStream as TokenStream2, TokenTree};
use quote::quote;
use syn::{parse_macro_input, ItemTrait};

mod types;
mod apply;
mod parse;
mod codegen;

use codegen::generate_impl;
use parse::{parse_item, Cursor};
use types::{reset_fresh_counter, Op};

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
/// - `TraitName<trait-泛型>` — trait 的泛型参数与关联类型绑定
/// - 目标类型 — 用 `[]` 包裹表示并列，用 `^`/`-` 表示泛型应用
///
/// ## 示例
///
/// ```ignore
/// #[batch_impl(usize, isize)]
/// trait Numeric {}
///
/// #[batch_impl(<T> Vec<T>)]
/// trait Collection {}
///
/// #[batch_impl(<T> FromValue<T> i32 { fn wrap(_: T) -> Self { 0 } })]
/// trait FromValue<T> { fn wrap(val: T) -> Self; }
/// ```
#[proc_macro_attribute]
pub fn batch_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    reset_fresh_counter();
    let trait_item = parse_macro_input!(item as ItemTrait);
    let trait_name = trait_item.ident.clone();
    let attr_vec = TokenStream2::from(attr).into_iter().collect::<Vec<_>>();
    let trait_name_ts: TokenStream2 = quote![#trait_name];
    let mut cursor = Cursor::new(&attr_vec);
    let impls = parse_batch_trait_entry(
        &mut cursor, Op::Comma, &trait_name_ts, &trait_name,
        trait_item.unsafety.is_some(), Some(trait_item),
    );
    impls.into()
}

/// 对已声明的 trait 批量生成 `impl` 块的函数式宏。
///
/// 语法：`unsafe? Trait路径: impl-specs;`，以 `;` 分隔多个 trait 段。
/// 每段的 `:` 之后是 DSL 表达式，与 `#[batch_impl]` 接受相同的语法。
///
/// ## 示例
///
/// ```ignore
/// trait A {}
/// trait B<T> {}
/// mod foo { pub trait C {} }
///
/// batch_trait!(
///     A: usize, isize;
///     B: <T> B<T> Vec<T>;
///     foo::C: u32;
///     unsafe UnsafeTrait: usize
/// );
/// ```
#[proc_macro]
pub fn batch_trait(input: TokenStream) -> TokenStream {
    reset_fresh_counter();
    let tokens = TokenStream2::from(input).into_iter().collect::<Vec<_>>();
    let mut cursor = Cursor::new(&tokens);
    let mut result = quote![];
    loop {
        // 跳过前导 `;`（允许连续多个分号、尾随分号）
        while cursor.is_punct(';') {
            cursor.bump();
        }
        if cursor.at_end() {
            break;
        }

        // `unsafe` 前缀：标记该段所有 impl 为 unsafe impl
        let is_unsafe = if matches!(cursor.peek(), Some(TokenTree::Ident(id)) if *id == "unsafe") {
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
                    if matches!(cursor.peek_at(1), Some(TokenTree::Punct(p2)) if p2.as_char() == ':') {
                        cursor.bump();
                        cursor.bump();
                    } else {
                        break;
                    }
                }
                _ => cursor.bump(),
            }
        }
        let trait_path = cursor.slice_since(path_start);
        if trait_path.is_empty() {
            result.extend(generate_compile_error("batch_trait! 中期望 trait 名称"));
            break;
        }
        let trait_full_path = match extract_trait_path(trait_path) {
            Ok(path) => path,
            Err(e) => { result.extend(e); break; }
        };
        let trait_last_ident = match extract_last_ident(trait_path) {
            Ok(ident) => ident,
            Err(e) => { result.extend(e); break; }
        };
        if !cursor.is_punct(':') {
            result.extend(generate_compile_error(
                "batch_trait! 中期望 ':' 分隔 trait 名称和 impl-specs",
            ));
            break;
        }
        cursor.bump();
        let impl_code = parse_batch_trait_entry(
            &mut cursor, Op::Semi, &trait_full_path,
            trait_last_ident, is_unsafe, None,
        );
        result.extend(impl_code);
    }
    result.into()
}

/// 共享驱动：从游标解析 impl-specs，展开并列列表，生成 impl 块。
///
/// `top_level` 控制顶层优先级：
/// - `Op::Comma` 用于 `#[batch_impl]`（整个参数按 `,` 分隔）
/// - `Op::Semi` 用于 `batch_trait!` 的单段 specs（按 `,` 分隔，遇到 `;` 段落边界停止）
///
/// 展开阶段通过 BFS 工作清单把 `Ty::Array`（并列列表）逐层摊平为叶子 `Ty`，
/// 再对每个叶子调用 `generate_impl` 生成对应的 impl 块。
fn parse_batch_trait_entry(
    cursor: &mut Cursor,
    top_level: Op,
    trait_full_path: &TokenStream2,
    trait_last_ident: &Ident,
    is_unsafe_trait: bool,
    start_trait: Option<ItemTrait>,
) -> TokenStream2 {
    let mut tys = vec![];
    while let Some(ty) = parse_item(cursor, top_level, Some(trait_last_ident)) {
        let mut queue = vec![ty];
        while let Some(item) = queue.pop() {
            match item.expand() {
                Ok(expanded) => {
                    for e in expanded.into_iter().rev() {
                        queue.push(e);
                    }
                }
                Err(leaf) => tys.push(leaf),
            }
        }
    }
    let mut impls = start_trait.map_or(quote![], |t| quote![#t]);
    for t in tys {
        impls.extend(generate_impl(t, trait_full_path, is_unsafe_trait));
    }
    impls
}

/// 从 trait 路径 token 序列中提取完整路径（用于输出 `impl ... for`）
fn extract_trait_path(trait_path: &[TokenTree]) -> Result<TokenStream2, TokenStream2> {
    let path: TokenStream2 = trait_path.iter().cloned().collect();
    if path.is_empty() {
        Err(generate_compile_error("batch_trait! 中期望 trait 名称"))
    } else {
        Ok(path)
    }
}

/// 从 trait 路径 token 序列中提取最后一个标识符（用作 trait_name 匹配）
fn extract_last_ident(trait_path: &[TokenTree]) -> Result<&Ident, TokenStream2> {
    trait_path
        .iter()
        .filter_map(|tt| if let TokenTree::Ident(id) = tt { Some(id) } else { None })
        .next_back()
        .ok_or_else(|| generate_compile_error("batch_trait! 中期望标识符作为 trait 名称"))
}

/// 构造 `compile_error!(msg)` 用于编译期报错
fn generate_compile_error(msg: &str) -> TokenStream2 {
    quote! { compile_error!(#msg); }
}
