//! 宏入口的共享实现：属性宏展开、`batch_trait!` 分段展开、公共管线。
//!
//! 错误机制分工：本层（入口层）用 `Result<_, TokenStream>` 经 `?` 传播，
//! 错误统一由 `compile_error_str` 构造；DSL 解析层（parse/apply/codegen）
//! 用 `Ty::Error` 在 AST 链中透传——两种机制服务于不同层级，不合并。
//!
//! 公共管线 [`run_pipeline`] = DSL 解析/展开 → 生成 impl → 尖括号组还原。
//! `angle_collect` 与裸 `where` 改写**不进入**管线：配对是破坏性的
//! （已配对组再次收集会被当真实 None 组扁平化），且 where 改写必须先于
//! `A<>` 展开（谓词里的 `Foo<>` 需透传，不得误展开）——二者由两个入口
//! 按序各调用一次。

use proc_macro2::{Ident, TokenStream, TokenTree};
use quote::quote;
use syn::ItemTrait;

use crate::ast::{Op, reset_fresh_counter};
use crate::batch_trait_entry::parse_batch_trait_entry;
use crate::diagnostic::compile_error_str;
use crate::empty_generics::expand_empty_trait_generics;
use crate::preprocess::{angle_collect, expand_tokens, render_angles, where_process};
use crate::scan::Cursor;
use crate::trait_bounds::TraitBounds;

/// 公共管线：DSL 解析/展开 → 生成 impl → 尖括号组还原。
///
/// `tokens` 须已通过 `angle_collect` 配对与裸 `where` 改写（见模块文档）；
/// `top_level` 控制 spec 列表的停止语义。错误经 `Err` 返回 `compile_error!` 流。
fn run_pipeline(
    tokens: &[TokenTree], top_level: Op, trait_full_path: &TokenStream,
    trait_last_ident: &Ident, is_unsafe: bool, start_trait: Option<ItemTrait>,
    trait_bounds: &TraitBounds,
) -> Result<TokenStream, TokenStream> {
    let mut cursor = Cursor::new(tokens);
    let impls = parse_batch_trait_entry(
        &mut cursor,
        top_level,
        trait_full_path,
        trait_last_ident,
        is_unsafe,
        start_trait,
        trait_bounds,
    );
    // 出口转换：尖括号组还原为 `<...>` 扁平 token（rustc 只见扁平）
    Ok(render_angles(impls))
}

/// 两个属性宏的共享实现（错误经 `compile_error!` token 流返回）
pub(crate) fn expand_attr_macro(
    attr: proc_macro::TokenStream, trait_item: ItemTrait, include_trait: bool,
) -> Result<proc_macro::TokenStream, TokenStream> {
    reset_fresh_counter();
    let trait_name = trait_item.ident.clone();
    let attr_vec = TokenStream::from(attr).into_iter().collect::<Vec<_>>();
    // 入口转换：真实 None 组扁平化（宏变量展开产物，内容即 DSL token）
    // + 扁平 `<...>` 配对为尖括号组（`->` 箭头不参与）——下游解析不再管 `<>` 深度
    let attr_vec = angle_collect(&attr_vec)?;

    // `#[batch_impl_only]` 专属：attr 起首若是 `# Path: ` 形式
    // （`#` + `Ident (:: Ident)*` + `:`），则把该路径作为外部 trait 路径，
    // 余下 attr 作为 DSL spec。`#[batch_impl]` 不支持此前缀
    // （它输出本地 trait 定义，路径前缀无意义）。
    let (trait_full_path, trait_last_ident, rest_tokens) = if !include_trait {
        match crate::path_prefix::try_parse_path_prefix(&attr_vec) {
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

    let expanded = crate::consts::expand_consts(&rest_tokens, None)?;
    let expanded =
        expand_tokens(&mut Cursor::new(&expanded), &trait_item, &trait_full_path)?;
    // 裸 `where 谓词 {body}` 新语法 → 统一改写为旧式 `where{谓词}`
    // （先于 `A<>` 展开：谓词里的 `Foo<>` 须透传，不得照抄展开）
    let expanded = where_process(&mut Cursor::new(&expanded))?;
    let is_unsafe = trait_item.unsafety.is_some();
    let trait_bounds = crate::trait_bounds::extract_trait_bounds(&trait_item);
    // `A<>`：trait 泛型照抄（实参与 bound 全部来自 trait 定义，含 where 谓词），
    // 展开产物与手写完全等价。
    let expanded =
        expand_empty_trait_generics(&expanded, &trait_item, &trait_bounds)?;
    let start_trait = if include_trait { trait_item.into() } else { None };
    run_pipeline(
        &expanded,
        Op::Comma,
        &trait_full_path,
        &trait_last_ident,
        is_unsafe,
        start_trait,
        &trait_bounds,
    )
    .map(Into::into)
}

/// `batch_trait!` 的实际展开（错误经 `compile_error!` token 流返回）
pub(crate) fn expand_batch_trait(
    input: proc_macro::TokenStream,
) -> Result<proc_macro::TokenStream, TokenStream> {
    reset_fresh_counter();
    let tokens = TokenStream::from(input).into_iter().collect::<Vec<_>>();
    // 全局预处理：配对尖括号组 + 用户常量定义段收集/引用替换 + 裸 where 改写
    // （分段前一次完成）
    let tokens = angle_collect(&tokens)?;
    let (tokens, user_consts) = crate::consts::collect_user_consts(&tokens)?;
    let tokens = crate::consts::expand_consts(&tokens, Some(&user_consts))?;
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

        // 收集 trait 路径（遇到 `:` 停止；`::` 路径分隔符一并收集）。
        // 尖括号已由 angle_collect 配对为不透明组，无需跟踪 `<>` 深度。
        let path_start = cursor.pos();
        while let Some(token) = cursor.peek() {
            match token {
                TokenTree::Punct(p) if p.as_char() == ':' => {
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
            return Err(compile_error_str("batch_trait! 中期望 trait 名称"));
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
                    return Err(compile_error_str(
                        "batch_trait! 中期望标识符作为 trait 名称",
                    ));
                }
            };
        if !cursor.is_punct(':') {
            return Err(compile_error_str(
                "batch_trait! 中期望 ':' 分隔 trait 名称和 impl-specs",
            ));
        }
        cursor.bump();
        // 段边界 = 首个深度 0 的 `;`（不消费，由循环头部跳过）
        let spec = cursor.take_segment(&[';']);
        result.extend(run_pipeline(
            spec,
            Op::Comma,
            &trait_full_path,
            trait_last_ident,
            is_unsafe,
            None,
            // batch_trait! 无 trait 定义，无法继承泛型 bound
            &Default::default(),
        )?);
    }
    Ok(result.into())
}
