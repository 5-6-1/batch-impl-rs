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

use crate::analyze::TraitBounds;
use crate::ast::{Op, reset_fresh_counter};
use crate::preprocess::{
    angle_collect, expand_empty_trait_generics, expand_tokens, render_angles,
    where_process,
};
use crate::util::{Cursor, compile_error_str};

use crate::entry::driver::parse_batch_trait_entry;

pub(crate) mod driver;
pub(crate) mod path_prefix;

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
    // 出口转换：尖括号组还原为 `<...>` 扁平（见 render_angles）
    Ok(render_angles(impls))
}

/// 两个属性宏的共享实现（错误经 `compile_error!` token 流返回）
/// 参数用 proc_macro2 类型：单元测试（fuzz）可直接调用而无需 proc-macro 运行时；
/// 属性宏入口（lib.rs）在展开时转换。
pub(crate) fn expand_attr_macro(
    attr: TokenStream, trait_item: ItemTrait, include_trait: bool,
) -> Result<TokenStream, TokenStream> {
    reset_fresh_counter();
    let trait_name = trait_item.ident.clone();
    let attr_vec = attr.into_iter().collect::<Vec<_>>();

    // `#[batch_impl_only]` 专属：attr 起首若是 `# Path: ` 形式
    // （`#` + `Ident (:: Ident)*` + `:`），则把该路径作为外部 trait 路径，
    // 余下 attr 作为 DSL spec。`#[batch_impl]` 不支持此前缀
    // （它输出本地 trait 定义，路径前缀无意义）。
    // 提前到 `@` 展开前：`@trait` 需要 trait_full_path（batch_impl_only
    // 展开为外部路径，batch_impl 为本地名）。
    let (trait_full_path, trait_last_ident, rest_tokens) = if !include_trait {
        match crate::entry::path_prefix::try_parse_path_prefix(&attr_vec) {
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

    // 宏元层最外：`@` 常量展开（纯词法替换）先于 `<>` 配对——
    // 展开产物可能含扁平 `<...>`（如 `@map = HashMap<u32, String>` 的值），
    // 必须由后续 angle_collect 统一配对；反序则 `Vec<@inner>` 的 `@inner`
    // 被配对进 `<>` 组、expand_consts 不进入组而残留（实测编译错）。
    let rest_tokens = crate::preprocess::expand_consts(
        &rest_tokens,
        crate::preprocess::ConstCtx::Attribute {
            trait_def: &trait_item,
            trait_full_path: &trait_full_path,
        },
    )?;
    // 入口转换：None 组扁平化 + `<...>` 配对（见 angle_collect）
    let rest_tokens = angle_collect(&rest_tokens)?;

    let expanded = expand_tokens(
        &mut Cursor::new(&rest_tokens),
        &trait_item,
        &trait_full_path,
    )?;
    // 裸 `where 谓词 {body}` 新语法 → 统一改写为旧式 `where{谓词}`
    // （先于 `A<>` 展开：谓词里的 `Foo<>` 须透传，不得照抄展开）
    let expanded = where_process(&mut Cursor::new(&expanded))?;
    let is_unsafe = trait_item.unsafety.is_some();
    let trait_bounds = crate::analyze::extract_trait_bounds(&trait_item);
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
}

/// 段级 `@trait` → 本段 trait 完整路径（batch_trait! 专用；常量值
/// `<T>@trait<T>` 经懒展开保留 `@trait`，此处按段替换——多段各用自己的名）。
fn replace_segment_trait(
    tokens: Vec<TokenTree>, trait_full_path: &TokenStream,
) -> Result<Vec<TokenTree>, TokenStream> {
    let mut out = vec![];
    let mut i = 0;
    while i < tokens.len() {
        if let TokenTree::Punct(p) = &tokens[i]
            && p.as_char() == '@'
            && let Some(TokenTree::Ident(id)) = tokens.get(i + 1)
            && id == "trait"
        {
            out.extend(trait_full_path.clone());
            i += 2;
        } else {
            out.push(tokens[i].clone());
            i += 1;
        }
    }
    Ok(out)
}

/// `batch_trait!` 的实际展开（错误经 `compile_error!` token 流返回）
pub(crate) fn expand_batch_trait(
    input: proc_macro::TokenStream,
) -> Result<proc_macro::TokenStream, TokenStream> {
    reset_fresh_counter();
    let tokens = TokenStream::from(input).into_iter().collect::<Vec<_>>();
    // 全局预处理：`@` 常量（宏元层最外）→ 尖括号配对 → 裸 where 改写
    // （分段前一次完成；`@` 先于配对：展开产物可能含扁平 `<...>`，
    //  须由 angle_collect 统一配对——反序则 `Vec<@inner>` 的 `@inner`
    //  进组后不被展开，实测编译错）
    let (tokens, user_consts) = crate::preprocess::collect_user_consts(&tokens)?;
    let tokens = crate::preprocess::expand_consts(
        &tokens,
        crate::preprocess::ConstCtx::Trait { user_table: &user_consts },
    )?;
    let tokens = angle_collect(&tokens)?;
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
        let spec = cursor.take_segment(&[';']).to_vec();
        // 段级 `@trait` 替换：batch_trait! 的 `@trait` 在常量阶段保留原样
        // （多段每段 trait 名不同），此处展开为本段 trait 完整路径——
        // `@type_t=<T>@trait<T>` 跨段复用场景（`A: @type_t ...` / `B: @type_t ...`）。
        let spec = replace_segment_trait(spec, &trait_full_path)?;
        result.extend(run_pipeline(
            &spec,
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
