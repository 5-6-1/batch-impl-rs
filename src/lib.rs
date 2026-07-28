#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))]
use proc_macro2::{Spacing, TokenStream, TokenTree};
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

use batch_trait_entry::parse_batch_trait_entry;

use diagnostic::compile_error_str;
use scan::Cursor;
use types::{Op, reset_fresh_counter};

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
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    expand_attr_macro(attr, item, true)
}

/// 与 `#[batch_impl]` 相同，但丢弃 trait 定义本身，只输出 `impl` 块。
///
/// 用于 trait 已在别处定义、只需批量生成 impl 的场景。
/// 语法与 `#[batch_impl]` 完全一致。
///
/// ## 示例
///
/// ```
/// trait Greet { fn hello(&self) -> &str; }
///
/// #[batch_impl_only(usize #hello{"hi"})]
/// trait Greet { fn hello(&self) -> &str; } // 此 trait 定义被丢弃，不影响已有的定义
/// // 这样写而不用batch_trait是为了使用指令系统，建议按trait定义处按原样写
/// ```
#[proc_macro_attribute]
pub fn batch_impl_only(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    expand_attr_macro(attr, item, false)
}

/// 两个属性宏的共享实现
fn expand_attr_macro(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
    include_trait: bool,
) -> proc_macro::TokenStream {
    reset_fresh_counter();
    let trait_item = parse_macro_input!(item as ItemTrait);
    let trait_name = trait_item.ident.clone();
    let attr_vec = TokenStream::from(attr).into_iter().collect::<Vec<_>>();

    // `#[batch_impl_only]` 专属：attr 起首若是 `# Path: ` 形式
    // （`#` + `Ident (:: Ident)*` + `:`），则把该路径作为外部 trait 路径，
    // 余下 attr 作为 DSL spec。`#[batch_impl]` 不支持此前缀
    // （它输出本地 trait 定义，路径前缀无意义）。
    let (trait_full_path, trait_last_ident, rest_tokens) =
        if !include_trait {
            match path_prefix::try_parse_path_prefix(&attr_vec) {
                Some((path, last_ident, rest)) => {
                    // 路径前缀的 last ident 必须与本地 dummy trait 名一致，
                    // 否则后续 DSL 中的 `Trait<T>` 匹配会失败。
                    match last_ident {
                        Some(id) if id == trait_name => {
                            let path_ts: TokenStream =
                                path.into_iter().collect();
                            // 此处借用本地 trait_name 作为匹配标识
                            // （已校验与路径末段同名）。
                            (path_ts, trait_name.clone(), rest)
                        },
                        Some(id) => {
                            let msg = format!(
                                "batch-impl: 路径前缀 `#...{}` \
                                 的末尾标识符与 trait 名 `{}` \
                                 不一致；二者必须相同",
                                id, trait_name,
                            );
                            return compile_error_str(&msg).into();
                        },
                        None => {
                            let msg = "batch-impl: 路径前缀 `#` 后 \
                                 期望至少一个标识符作为 trait 路径";
                            return compile_error_str(msg).into();
                        },
                    }
                },
                None => {
                    let ts = quote![#trait_name];
                    (ts, trait_name.clone(), attr_vec.clone())
                },
            }
        } else {
            let ts = quote![#trait_name];
            (ts, trait_name.clone(), attr_vec.clone())
        };

    let mut cursor = Cursor::new(&rest_tokens);
    let expanded =
        match preprocess::expand_tokens(&mut cursor, &trait_item) {
            Ok(tokens) => tokens,
            Err(err) => return err.into(),
        };
    cursor = Cursor::new(&expanded);
    let is_unsafe = trait_item.unsafety.is_some();
    let start_trait = if include_trait {
        Some(trait_item)
    } else {
        None
    };
    let impls = parse_batch_trait_entry(
        &mut cursor,
        Op::Comma,
        &trait_full_path,
        &trait_last_ident,
        is_unsafe,
        start_trait,
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
/// ```
/// trait A {}
/// trait B<T> {}
/// mod foo { pub trait C {} }
/// unsafe trait UnsafeTrait{}
///
/// batch_trait!(
///     A: usize, isize;
///     B: <T> B<T> Vec<T>;
///     foo::C: u32;
///     unsafe UnsafeTrait: usize
/// );
/// ```
#[proc_macro]
pub fn batch_trait(
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    reset_fresh_counter();
    let tokens = TokenStream::from(input).into_iter().collect::<Vec<_>>();
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
                },
                TokenTree::Punct(p) if p.as_char() == '>' => {
                    depth -= 1;
                    cursor.bump();
                },
                TokenTree::Punct(p)
                    if p.as_char() == ':' && depth == 0 =>
                {
                    if matches!(cursor.peek_at(1), Some(TokenTree::Punct(p2)) if p.spacing()==Spacing::Joint && p2.as_char() == ':')
                    {
                        cursor.bump();
                        cursor.bump();
                    } else {
                        break;
                    }
                },
                _ => cursor.bump(),
            }
        }
        let trait_path = cursor.slice_since(path_start);
        if trait_path.is_empty() {
            result.extend(compile_error_str(
                "batch_trait! 中期望 trait 名称",
            ));
            break;
        }
        // trait 完整路径：原样收集 trait_path 的 token 流即可
        let trait_full_path = trait_path.iter().cloned().collect();
        // 取路径中的最后一个标识符作为 `trait_name` 匹配用
        let trait_last_ident = match trait_path
            .iter()
            .filter_map(|tt| {
                if let TokenTree::Ident(id) = tt {
                    Some(id)
                } else {
                    None
                }
            })
            .next_back()
        {
            Some(ident) => ident,
            None => {
                result.extend(compile_error_str(
                    "batch_trait! 中期望标识符作为 trait 名称",
                ));
                break;
            },
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
        );
        result.extend(impl_code);
    }
    result.into()
}


