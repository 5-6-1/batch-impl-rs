use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, ItemTrait};

mod core;

use core::codegen::generate_impl;
use core::parser::parse_top_level;
use core::types::{err, ParseResult};

// ===========================================================================
// #[batch_impl(...)]
// ===========================================================================

/// 为 trait 批量生成 impl 块的属性宏。
///
/// # 语法概览
///
/// ```text
/// #[batch_impl( impl-spec [, impl-spec]* [ { body }]? )]
/// impl-spec = [ <impl-泛型> ] [ Trait名<trait-泛型> ] 目标 [ { body } ]
/// ```
///
/// # `^` 运算符（右结合）
///
/// `A^B^C = A^(B^C)`
///
/// | 写法 | 展开 |
/// |------|------|
/// | `&^T` | `&T` |
/// | `&mut^T` | `&mut T` |
/// | `self^T` | `T` |
/// | `A^B` | `A<B>` |
/// | `A^<X,Y>` | `A<X,Y>` |
/// | `[A1,A2]^B` | `A1<B>, A2<B>` |
/// | `A^[B1,B2]` | `A<B1>, A<B2>` |
/// | `[A1,A2]^[B1,B2]` | 笛卡尔积 `A1<B1>, A1<B2>, A2<B1>, A2<B2>` |
/// | `Box^Box^T` | `Box<Box<T>>` |
/// | `Box^[Box^T]` | `Box<[Box<T>]>` |
/// | `HashMap<K>^V` | `HashMap<K, V>`（预填泛型追加） |
/// | `[HashMap<K>, Vec<K>]^V` | `HashMap<K, V>, Vec<K, V>` |
///
/// # 元组 `^`（追加/生成）
///
/// 追加（右侧是类型）：
/// | 写法 | 展开 |
/// |------|------|
/// | `()^T` | `(T,)` |
/// | `(A,B)^T` | `(A, B, T)` |
///
/// 生成（右侧是数字/范围）：
/// | 写法 | 展开 |
/// |------|------|
/// | `()^N` | `(), (X,), (X,X), ...` |
/// | `(T)^N` | `(), (T,), (T,T), ...` |
/// | `(<tr>)^N` | `(), (A:tr,), (A:tr,B:tr), ...` |
/// | `^M..N` | 长度 M 到 N-1 |
/// | `^M..=N` | 长度 M 到 N |
///
/// 笛卡尔积生成（前缀含逗号）：
/// | 写法 | 展开 |
/// |------|------|
/// | `(T1,T2)^N` | 长度 0..N-1 的所有 T1/T2 组合 |
/// | `(<tr>,T)^N` | 带 bound 的泛型+固定类型组合 |
///
/// # `-` 运算符（左结合）
///
/// `-` 与 `^` 语义完全相同，仅结合方向不同：`A-B = A^B`，`A-B-C = (A-B)-C`
///
/// | 写法 | 展开 |
/// |------|------|
/// | `Vec-u32` | `Vec<u32>`（同 `Vec^u32`） |
/// | `HashMap-u32-String` | `HashMap<u32, String>`（左结合，预填泛型追加） |
/// | `()-[A,B]` | `(A,), (B,)` |
/// | `()-[A,B]-[C,D]` | `(A,C), (A,D), (B,C), (B,D)` |
/// | `()-[A]-[B]-[C]` | `(A,B,C)` |
///
/// # 优先级
///
/// `^` 高于 `-`，`,` 最低
///
/// # 其他规则
///
/// - **`[]` 歧义**：有逗号是并列列表，无逗号是切片类型
/// - **`()` 歧义**：`()=空元组`, `(A,)=单元素元组`, `(A)=分组（非元组）`
/// - **trait 泛型必须显式**：trait 有泛型时必须写 `Trait名<T>`
/// - **泛型继承**：嵌套 `[...]` 中子项不写泛型则继承父级，写了则追加并去重
/// - **body 继承**：列表级 `{...}` 被所有子项共享；子项 `{...}` 覆盖列表级
/// - **目标类型透传**：不解析，原样透传
///
/// # 基本示例
///
/// ```
/// # use batch_impl::batch_impl;
/// #[batch_impl(usize, isize)]
/// trait Numeric {}
/// ```
///
/// # 类型标注 / const 泛型 / 生命周期
///
/// ```
/// # use batch_impl::batch_impl;
/// #[batch_impl(<T: Clone + std::fmt::Debug> Vec<T>)]
/// trait DebugClone {}
///
/// #[batch_impl(<const N: usize> [i32; N])]
/// trait FixedSize {}
///
/// #[batch_impl(<'a, T: 'a> &'a T)]
/// trait RefTrait {}
/// ```
///
/// # trait 自身带泛型参数
///
/// ```
/// # use batch_impl::batch_impl;
/// #[batch_impl(<T> FromValue<T> i32 {
///     fn wrap(_val: T) -> Self { 0 }
/// })]
/// trait FromValue<T> { fn wrap(val: T) -> Self; }
///
/// #[batch_impl(<T> Wrapper<Vec<T>> Vec<T> {
///     fn inner(self) -> Vec<T> { self }
/// })]
/// trait Wrapper<C> { fn inner(self) -> C; }
/// ```
///
/// > **必须显式写 `Trait名<泛型>`**，否则生成的 impl 缺少 trait 泛型。
///
/// # 并列列表
///
/// ```
/// # use batch_impl::batch_impl;
/// #[batch_impl([usize, isize, f32] {
///     fn tag(&self) -> &'static str { "number" }
/// })]
/// trait Tagged { fn tag(&self) -> &'static str; }
///
/// // 嵌套泛型合并（<U> 追加到父级 <T>，去重）
/// # use std::collections::HashMap;
/// #[batch_impl(<T> Describe<T> [Vec<T>, <U> HashMap<T, U>] {
///     fn describe(&self) -> String { format!("len={}", self.len()) }
/// })]
/// trait Describe<T> { fn describe(&self) -> String; }
/// // → impl<T>    Describe<T> for Vec<T>
/// // → impl<T, U> Describe<T> for HashMap<T, U>
/// ```
///
/// # `^` 运算符示例
///
/// ```
/// # use batch_impl::batch_impl;
/// #[batch_impl([&, self]^u32)]
/// trait RefOrOwned {}
///
/// # use std::collections::HashMap;
/// #[batch_impl(HashMap^<u32, i32>)]
/// trait MapMarker {}
/// ```
///
/// # 多项独立 body
///
/// ```
/// # use batch_impl::batch_impl;
/// #[batch_impl(
///     usize  { fn id(&self) -> usize { *self }      },
///     String { fn id(&self) -> usize { self.len() }  }
/// )]
/// trait Identifiable { fn id(&self) -> usize; }
/// ```
///
/// # 复杂类型透传
///
/// ```
/// # use batch_impl::batch_impl;
/// #[batch_impl((i32, String), &str, Box<dyn std::fmt::Display>, dyn Fn() + Send + Sync)]
/// trait ComplexMarker {}
/// ```
///
/// # 设计约束
///
/// - **where 子句**：不在 DSL 内，复杂 bound 写在 trait 定义自身
/// - **`for<'a>` 高阶 trait bound**：where 子句式范畴；类型内部走 token 透传
/// - **`<>` 多参不能在 `[]` 内**：Rust `[<` 解析歧义，拆成独立表达式
#[proc_macro_attribute]
pub fn batch_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    let trait_item = parse_macro_input!(item as ItemTrait);
    let trait_name = &trait_item.ident;
    let name_ts = quote! { #trait_name };

    // 检测是否是 unsafe trait
    let is_unsafe_trait = trait_item.unsafety.is_some();

    let attr_ts: TokenStream2 = attr.into();
    let specs = parse_top_level(attr_ts, trait_name, trait_name.span());

    let mut output = TokenStream2::new();
    match specs {
        ParseResult::Ok(specs) => {
            for mut spec in specs {
                // 如果是 unsafe trait，所有 impl 都标记为 unsafe
                if is_unsafe_trait {
                    spec.is_unsafe = true;
                }
                output.extend(generate_impl(&spec, name_ts.clone()));
            }
        }
        ParseResult::Err(e) => output.extend(e),
    }

    (quote! { #trait_item #output }).into()
}

// ===========================================================================
// batch_trait!(Trait: impl-specs; ...)
// ===========================================================================

/// 对已声明的 trait 批量生成 impl 块的函数式宏。
///
/// # 语法
///
/// ```text
/// batch_trait!(Trait路径: impl-specs; Trait路径: impl-specs)
/// ```
///
/// `:` 前是 trait 路径（支持 `sub::MyTrait`），`:` 后是 `#[batch_impl]` 同款参数，
/// `;` 分隔不同的 trait。
///
/// ```
/// # use batch_impl::batch_trait;
/// trait A {}
/// trait B<T> {}
/// mod foo { pub trait C {} }
///
/// batch_trait!(
///     A: usize, isize;
///     B: <T> B<T> Vec<T>;
///     foo::C: u32
/// );
/// // → impl A for usize {}  +  impl A for isize {}
/// // → impl<T> B<T> for Vec<T> {}
/// // → impl foo::C for u32 {}
/// ```
///
/// **注意**：trait 有泛型时，必须在 `:` 后的 impl-spec 中显式写出 `Trait名<泛型>`。
#[proc_macro]
pub fn batch_trait(input: TokenStream) -> TokenStream {
    let input_ts: TokenStream2 = input.into();
    let span = core::utils::tokens_span(&input_ts);

    let segments = match core::utils::split_by(input_ts, ';', span) {
        Ok(s) => s,
        Err(e) => return e.into(),
    };

    let mut output = TokenStream2::new();
    for seg in segments {
        let seg_vec: Vec<proc_macro2::TokenTree> = seg.into_iter().collect();
        if seg_vec.is_empty() {
            continue;
        }

        let colon = match core::utils::find_top_level_colon(&seg_vec) {
            Some(p) => p,
            None => {
                output.extend(err(
                    span,
                    "batch_trait! 缺少 `:`（格式：Trait名: impl规格，如 A: usize, isize）",
                ));
                continue;
            }
        };

        // 检测 unsafe 关键字
        let (is_unsafe, trait_start) = if colon > 0
            && matches!(&seg_vec[0], proc_macro2::TokenTree::Ident(id) if id == "unsafe")
        {
            (true, 1)
        } else {
            (false, 0)
        };

        let trait_path: TokenStream2 = seg_vec[trait_start..colon].iter().cloned().collect();
        let specs_ts: TokenStream2 = seg_vec[colon + 1..].iter().cloned().collect();

        let trait_ident = match syn::parse2::<syn::Path>(trait_path.clone()) {
            Ok(p) => p.segments.last().unwrap().ident.clone(),
            Err(e) => {
                output.extend(err(
                    span,
                    &format!(
                        "batch_trait! 中 `:` 左侧不是合法的路径: {}。示例: A, foo::Bar",
                        e
                    ),
                ));
                continue;
            }
        };

        match parse_top_level(specs_ts, &trait_ident, trait_ident.span()) {
            ParseResult::Ok(specs) => {
                for mut spec in specs {
                    if is_unsafe {
                        spec.is_unsafe = true;
                    }
                    output.extend(generate_impl(&spec, trait_path.clone()));
                }
            }
            ParseResult::Err(e) => output.extend(e),
        }
    }
    output.into()
}
