#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))]
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/tutorial.md"))]
// 库不使用任何 unsafe；缺失文档按错误拒绝（仅作用于 pub 项，内部 pub(crate) 不受限）。
#![forbid(unsafe_code)]
#![deny(missing_docs)]
// MSVC 链接器输出"正在创建库…和对象…"到 stdout，被 rustc 当 linker_messages 告警，
// 属于无害的 Windows 链接产物提示，全局抑制。
#![allow(linker_messages)]
// `delimiter!` 宏定义在 preprocess 顶部，经 `#[macro_use]` 导入 crate 根；
// 文本作用域要求其声明先于所有使用者（fuzz / parse / 本模块）。
#[macro_use]
pub(crate) mod preprocess;
#[cfg(test)]
mod fuzz;
use proc_macro2::{TokenStream, TokenTree};
use syn::{ItemTrait, parse_macro_input};

mod apply;
mod ast;
mod batch_trait_entry;
mod codegen;
mod diagnostic;
mod empty_generics;
mod expand;
mod parse;
mod path_prefix;
mod scan;
mod trait_bounds;

pub(crate) use expand::{expand_attr_macro, expand_batch_trait};
pub(crate) use trait_bounds::TraitBounds;

use diagnostic::compile_error_str;
use preprocess::{build_from_item, get_trait_item, parse_names_from_tokens};

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
    let tokens = match preprocess::angle_collect(&tokens) {
        Ok(v) => v,
        Err(e) => return e.into(),
    };
    // 形如：`(add, inc) {*self+1} trait AddInc {...}`
    let Some(TokenTree::Group(names_group)) = tokens.first() else {
        return compile_error_str(
            "batch-impl: batch_preprocess_test 期望 `(方法名列表){body} trait ...`",
        )
        .into();
    };
    if names_group.delimiter() != delimiter![()] {
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
    if body_group.delimiter() != delimiter![{}] {
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
    preprocess::render_angles(methods).into()
}
