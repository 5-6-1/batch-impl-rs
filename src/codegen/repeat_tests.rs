//! Repeat-block expansion tests (kept under the 350-line cap by living in
//! their own file): the `@(...)..` blocks drive rounds from the
//! variadic-segment lengths, with `@ident` name / `@N` cursor substitution.

use super::VarSeg;
use super::repeat::expand_repeat_blocks;
use proc_macro2::TokenStream;

fn segs() -> Vec<VarSeg> {
    vec![
        VarSeg { prefix: "A".into(), start: 0, len: 3 },
        VarSeg { prefix: "B".into(), start: 1, len: 2 },
    ]
}

fn expand(s: &str) -> Result<String, String> {
    let ts = s.parse::<TokenStream>().map_err(|e| e.to_string())?;
    expand_repeat_blocks(
        ts,
        &segs(),
        None,
        &crate::codegen::FreshCtx::new(&[], &Default::default()),
    )
    .map(|o| o.to_string())
    .map_err(|e| e.to_string())
}

/// A cursor-only block with **no** template segment: the fresh count drives
/// the rounds (`@N` → `N + i` per round).
fn expand_fresh(s: &str, n: usize) -> Result<String, String> {
    let ts = s.parse::<TokenStream>().map_err(|e| e.to_string())?;
    let binding = crate::ast::fresh::FreshRef {
        group: None,
        start: 0,
        end: crate::ast::fresh::FreshEnd::Open,
    };
    expand_repeat_blocks(
        ts,
        &[],
        Some(binding),
        &crate::codegen::FreshCtx::new(&fresh_names(n), &Default::default()),
    )
    .map(|o| o.to_string())
    .map_err(|e| e.to_string())
}

fn fresh_names(n: usize) -> Vec<TokenStream> {
    // Declaration carriers `@{0_i}` — the identity form the ctx parses.
    (0..n).map(|i| crate::ast::fresh::fresh_decl_tokens(0, i)).collect()
}

#[test]
fn fresh_count_drives_cursor_only_block() {
    // no segments at all — the fresh count (4) repeats the block
    assert_eq!(
        expand_fresh("@(args.@0,)..", 4).unwrap(),
        "args .0 , args .1 , args .2 , args .3 ,"
    );
}

#[test]
fn fresh_count_zero_empty() {
    // no segments and no fresh: the block contributes zero rounds (the
    // arity-0 impl of a `Fn()0..N` bound generator)
    assert_eq!(expand_fresh("@(args.@0,)..", 0).unwrap(), "");
}

#[test]
fn fresh_name_reference() {
    // `@@N` → the N-th fresh generic's name, **fixed** (not per-round):
    // `@@1` names the second fresh in every round
    assert_eq!(expand_fresh("@(@@1,)..", 3).unwrap(), "P1 , P1 , P1 ,");
}

#[test]
fn fresh_name_out_of_range() {
    assert!(expand_fresh("@(@@5,)..", 3).is_err());
}

#[test]
fn no_switch_no_segment_errors() {
    // without the fresh-binding switch (`impl{@0..}`), a cursor-only block
    // with no template segment errors — fresh-driven body modification is off
    let ts = "@(args.@0,)..".parse::<TokenStream>().unwrap();
    assert!(
        expand_repeat_blocks(
            ts,
            &[],
            None,
            &crate::codegen::FreshCtx::new(&[], &Default::default())
        )
        .is_err()
    );
}

#[test]
fn single_segment_rounds() {
    assert_eq!(
        expand("@(@A::f(&self.@0),)..").unwrap(),
        "@ { A_0 } :: f (& self .0) , @ { A_1 } :: f (& self .1) , @ { A_2 } :: f (& self .2) ,"
    );
}

#[test]
fn offset_start_name_numbering() {
    // B starts at leaf index 1: names B1, B2; `@1` cursor → 1, 2.
    assert_eq!(
        expand("@(@B::f(&self.@1),)..").unwrap(),
        "@ { B_1 } :: f (& self .1) , @ { B_2 } :: f (& self .2) ,"
    );
}

#[test]
fn multi_segment_parallel_rounds() {
    // Two equal-length segments drive the block: one shared cursor, each
    // round takes the i-th element of both.
    let segs = vec![
        VarSeg { prefix: "A".into(), start: 0, len: 2 },
        VarSeg { prefix: "B".into(), start: 2, len: 2 },
    ];
    let ts = "@(@A + @B,)..".parse::<TokenStream>().unwrap();
    let out = expand_repeat_blocks(
        ts,
        &segs,
        None,
        &crate::codegen::FreshCtx::new(&[], &Default::default()),
    )
    .unwrap()
    .to_string();
    assert_eq!(out, "@ { A_0 } + @ { B_2 } , @ { A_1 } + @ { B_3 } ,");
}

#[test]
fn unequal_segment_lengths_error() {
    let segs = vec![
        VarSeg { prefix: "A".into(), start: 0, len: 3 },
        VarSeg { prefix: "B".into(), start: 1, len: 2 },
    ];
    let ts = "@(@A + @B,)..".parse::<TokenStream>().unwrap();
    assert!(
        expand_repeat_blocks(
            ts,
            &segs,
            None,
            &crate::codegen::FreshCtx::new(&[], &Default::default())
        )
        .is_err()
    );
}

#[test]
fn nested_cartesian() {
    // Outer rounds A0/A1/A2; each inner runs B over 1,2. The outer
    // block body has no trailing comma (the inner block's own trailing
    // commas separate the B elements), so no double comma appears.
    let out = expand("@(@A::f(&self.@0) @(@B::g(&self.@1),)..)..").unwrap();
    assert_eq!(
        out,
        "@ { A_0 } :: f (& self .0) @ { B_1 } :: g (& self .1) , @ { B_2 } :: g (& self .2) , \
         @ { A_1 } :: f (& self .1) @ { B_1 } :: g (& self .1) , @ { B_2 } :: g (& self .2) , \
         @ { A_2 } :: f (& self .2) @ { B_1 } :: g (& self .1) , @ { B_2 } :: g (& self .2) ,"
    );
}

#[test]
fn no_trailing_separator_concatenates() {
    assert_eq!(expand("@(@A)..").unwrap(), "@ { A_0 } @ { A_1 } @ { A_2 }");
}

#[test]
fn inter_round_separator() {
    // `@(@A),..` — the comma sits between rounds, never after the last one
    // (the `$($A),*` form; `@(@A,)..` is the `$($A,)*` form)
    assert_eq!(expand("@(@A),..").unwrap(), "@ { A_0 } ,@ { A_1 } ,@ { A_2 }");
}

#[test]
fn inter_round_separator_arbitrary() {
    // any literal tokens work as the inter-round separator
    assert_eq!(expand("@(@A)+..").unwrap(), "@ { A_0 } +@ { A_1 } +@ { A_2 }");
    assert_eq!(expand("@(@A)::f()..").unwrap(), "@ { A_0 } :: f () @ { A_1 } :: f () @ { A_2 }");
}

#[test]
fn inter_round_separator_single_round() {
    // one round: no separator is emitted at all
    let segs = vec![VarSeg { prefix: "A".into(), start: 0, len: 1 }];
    let ts = "@(@A),..".parse::<TokenStream>().unwrap();
    let out = expand_repeat_blocks(
        ts,
        &segs,
        None,
        &crate::codegen::FreshCtx::new(&[], &Default::default()),
    )
    .unwrap()
    .to_string();
    assert_eq!(out, "@ { A_0 }");
}

#[test]
fn float_literal_at_path_fixed() {
    // `self.0.@0` tokenizes `0.` as a float literal; the fix splits it
    // so the cursor expands into `self.0.0`, `self.0.1`, ...
    let segs = vec![VarSeg { prefix: "A".into(), start: 0, len: 2 }];
    let ts = "@(@A::from(self.0.@0),)..".parse::<TokenStream>().unwrap();
    let out = expand_repeat_blocks(
        ts,
        &segs,
        None,
        &crate::codegen::FreshCtx::new(&[], &Default::default()),
    )
    .unwrap()
    .to_string();
    assert_eq!(out, "@ { A_0 } :: from (self . 0 . 0) , @ { A_1 } :: from (self . 0 . 1) ,");
}

#[test]
fn plain_body_passthrough() {
    let s = "fn combine (& self , rhs : & Self) -> Self { todo ! () }";
    assert_eq!(expand(s).unwrap(), s);
}

#[test]
fn declared_driver_cursor_only() {
    // `@A(self.@0,)..` — the driving segment declared up front, the
    // body uses only `@N` cursors
    let segs = vec![VarSeg { prefix: "A".into(), start: 0, len: 3 }];
    let ts = "@A(self.@0,)..".parse::<TokenStream>().unwrap();
    let out = expand_repeat_blocks(
        ts,
        &segs,
        None,
        &crate::codegen::FreshCtx::new(&[], &Default::default()),
    )
    .unwrap()
    .to_string();
    assert_eq!(out, "self .0 , self .1 , self .2 ,");
}

#[test]
fn cursor_only_single_segment() {
    // no declared driver and no inner `@ident`: the template's unique
    // segment provides the length
    let segs = vec![VarSeg { prefix: "A".into(), start: 0, len: 2 }];
    let ts = "@(self.@0,)..".parse::<TokenStream>().unwrap();
    let out = expand_repeat_blocks(
        ts,
        &segs,
        None,
        &crate::codegen::FreshCtx::new(&[], &Default::default()),
    )
    .unwrap()
    .to_string();
    assert_eq!(out, "self .0 , self .1 ,");
}

#[test]
fn cursor_only_multi_segment_errors() {
    // a cursor-only block with several template segments cannot pick a
    // length — reject instead of guessing
    let segs = vec![
        VarSeg { prefix: "A".into(), start: 0, len: 2 },
        VarSeg { prefix: "B".into(), start: 2, len: 2 },
    ];
    let ts = "@(self.@0,)..".parse::<TokenStream>().unwrap();
    assert!(
        expand_repeat_blocks(
            ts,
            &segs,
            None,
            &crate::codegen::FreshCtx::new(&[], &Default::default())
        )
        .is_err()
    );
}

#[test]
fn declared_driver_conflict_errors() {
    let segs = vec![
        VarSeg { prefix: "A".into(), start: 0, len: 2 },
        VarSeg { prefix: "B".into(), start: 2, len: 2 },
    ];
    let ts = "@A(@B::f(),)..".parse::<TokenStream>().unwrap();
    assert!(
        expand_repeat_blocks(
            ts,
            &segs,
            None,
            &crate::codegen::FreshCtx::new(&[], &Default::default())
        )
        .is_err()
    );
}

#[test]
fn bare_at_errors() {
    assert!(expand("x @ 0").is_err());
}

#[test]
fn unknown_segment_errors() {
    assert!(expand("@(@X::f(),)..").is_err());
}

#[test]
fn no_segments_no_fresh_empty() {
    // a cursor-only block with no template segments and no fresh contributes
    // zero rounds (not an error — the arity-0 impl of a bound generator)
    assert_eq!(expand_fresh("@(@0,)..", 0).unwrap(), "");
}
