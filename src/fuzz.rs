//! 解析器无 panic 属性的属性测试（proptest）。
//!
//! 库的承诺是"不因用户输入 panic"。用随机 token 序列喂给最危险的
//! 两个入口（`where_process` 裸 where 改写、`parse_item` DSL 解析），
//! 断言任意输入都不会 panic —— 即便结果是 `Err` / `None` 也接受。

use proc_macro2::{Delimiter, Group, Ident, Literal, Punct, Spacing, TokenTree};
use proptest::prelude::*;
use std::str::FromStr;

use crate::parse::parse_item;
use crate::scan::Cursor;
use crate::types::Op;
use crate::where_process::where_process;

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
        // 数字字面量（u8 范围内的 DSL 指数）
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
            prop::strategy::Just(Delimiter::Parenthesis),
            prop::strategy::Just(Delimiter::Bracket),
            prop::strategy::Just(Delimiter::Brace),
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
        let ts: Vec<TokenTree> = toks.iter().map(to_token).collect();
        let _ = where_process(&mut Cursor::new(&ts));
    }

    /// DSL 解析：任意 token 输入不 panic，且能正常推进到结束
    #[test]
    fn parse_no_panic(toks in tokens(3)) {
        let ts: Vec<TokenTree> = toks.iter().map(to_token).collect();
        let mut cursor = Cursor::new(&ts);
        while parse_item(&mut cursor, Op::Comma, None).is_some() {}
        prop_assert!(cursor.at_end());
    }
}
