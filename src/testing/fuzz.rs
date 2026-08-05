//! 无 panic 属性的属性测试（proptest）。
//!
//! 库的承诺是"不因用户输入 panic"。用随机 token 序列喂给危险入口，
//! 断言任意输入都不会 panic —— 即便结果是 `Err` / `None` / `compile_error!`
//! 也接受。覆盖：裸 where 改写、DSL 解析、以及**全管线**（指令预处理 →
//! where 改写 → 解析/展开 → 生成 impl，含 apply/expand/codegen）。

use proc_macro2::{
    Delimiter, Group, Ident, Literal, Punct, Spacing, TokenStream, TokenTree,
};
use proptest::prelude::*;
use std::str::FromStr;

use crate::ast::Op;
use crate::entry::expand_attr_macro;
use crate::parse::parse_item;
use crate::preprocess::where_process;
use crate::util::Cursor;

/// 可递归生成的 token 描述（Groups 里嵌套 Vec<Tok>，深度受限）
#[derive(Clone, Debug)]
enum Tok {
    Ident(&'static str),
    Literal(&'static str),
    Punct(char, Spacing),
    Group(Delimiter, Vec<Tok>),
}

/// 深度受限的 token 列表生成器（覆盖 DSL 关键字、运算符、括号嵌套）
fn tokens(depth: usize) -> impl Strategy<Value = Vec<Tok>> {
    let leaf = prop_oneof![
        // DSL / Rust 关键字与常见类型名
        prop::strategy::Just(Tok::Ident("usize")),
        prop::strategy::Just(Tok::Ident("isize")),
        prop::strategy::Just(Tok::Ident("Vec")),
        prop::strategy::Just(Tok::Ident("Box")),
        prop::strategy::Just(Tok::Ident("T")),
        prop::strategy::Just(Tok::Ident("where")),
        prop::strategy::Just(Tok::Ident("fn")),
        prop::strategy::Just(Tok::Ident("self")),
        prop::strategy::Just(Tok::Ident("unsafe")),
        // 数字字面量（小整数 DSL 指数）
        prop::strategy::Just(Tok::Literal("0")),
        prop::strategy::Just(Tok::Literal("1")),
        prop::strategy::Just(Tok::Literal("3")),
        // DSL 运算符与标点
        prop::strategy::Just(Tok::Punct('<', Spacing::Alone)),
        prop::strategy::Just(Tok::Punct('>', Spacing::Alone)),
        prop::strategy::Just(Tok::Punct('^', Spacing::Alone)),
        prop::strategy::Just(Tok::Punct('-', Spacing::Alone)),
        prop::strategy::Just(Tok::Punct(',', Spacing::Alone)),
        prop::strategy::Just(Tok::Punct(';', Spacing::Alone)),
        prop::strategy::Just(Tok::Punct(':', Spacing::Alone)),
        // Joint 的 `:` 可与下一个 `:` 拼成 `::`
        prop::strategy::Just(Tok::Punct(':', Spacing::Joint)),
        prop::strategy::Just(Tok::Punct('&', Spacing::Alone)),
        prop::strategy::Just(Tok::Punct('*', Spacing::Alone)),
        prop::strategy::Just(Tok::Punct('#', Spacing::Alone)),
        prop::strategy::Just(Tok::Punct('!', Spacing::Alone)),
        prop::strategy::Just(Tok::Punct('=', Spacing::Alone)),
    ];
    if depth == 0 {
        prop::collection::vec(leaf, 0..6).boxed()
    } else {
        let grouped = prop_oneof![
            prop::strategy::Just(delimiter![()]),
            prop::strategy::Just(delimiter![[]]),
            prop::strategy::Just(delimiter![{}]),
            // 真实 None 组模拟宏变量展开产物——angle_collect 应扁平化（内容即 DSL token）
            prop::strategy::Just(delimiter![none]),
        ]
        .prop_flat_map(move |d| {
            tokens(depth - 1).prop_map(move |inner| Tok::Group(d, inner))
        });
        prop::collection::vec(prop_oneof![leaf, grouped], 0..6).boxed()
    }
}

fn to_token(tok: &Tok) -> TokenTree {
    match tok {
        Tok::Ident(s) => Ident::new(s, proc_macro2::Span::call_site()).into(),
        Tok::Literal(s) => Literal::from_str(s).unwrap().into(),
        Tok::Punct(c, sp) => Punct::new(*c, *sp).into(),
        Tok::Group(d, inner) => {
            let stream = inner.iter().map(to_token).collect();
            Group::new(*d, stream).into()
        }
    }
}

proptest! {
    /// 裸 where 改写：任意 token 输入不 panic
    #[test]
    fn where_process_no_panic(toks in tokens(3)) {
        let ts = toks.iter().map(to_token).collect::<Vec<_>>();
        let _ = where_process(&mut Cursor::new(&ts));
    }

    /// DSL 解析：任意 token 输入不 panic，且能正常推进到结束
    #[test]
    fn parse_no_panic(toks in tokens(3)) {
        let ts = toks.iter().map(to_token).collect::<Vec<_>>();
        let mut cursor = Cursor::new(&ts);
        while parse_item(&mut cursor, Op::Comma, None).is_some() {}
        prop_assert!(cursor.at_end());
    }

    /// 全管线：走真实宏入口 `expand_attr_macro`（angle_collect → 常量展开 →
    /// 指令预处理 → where 改写 → `A<>` 照抄 → 解析/展开 → 生成 impl），
    /// 任意输入不 panic。用固定 dummy trait 作签名真相源；随机 token 里的指令
    /// 可能查不到 item（报 `compile_error!`）或产生非法类型（透传成垃圾），
    /// 均接受——承诺是"不 panic"。复用真实入口保证 fuzz 覆盖与线上完全
    /// 相同的路径（此前手写管线会漏掉常量展开与 `A<>` 照抄）。
    #[test]
    fn full_pipeline_no_panic(toks in tokens(3)) {
        let ts = toks.iter().map(to_token).collect::<TokenStream>();
        let trait_def: syn::ItemTrait = syn::parse_quote! {
            trait Fuzz { fn m(&self) -> u32; }
        };
        let _ = expand_attr_macro(ts, trait_def, false);
    }
}
