//! Repeat-block expansion tests (kept under the 350-line cap by living in
//! their own file): the `@(...)..` blocks drive rounds from the
//! variadic-segment lengths, with `@ident` element splicing (the `$(...)*`
//! semantics — the bound leaf subtree lands in the round's output) and
//! `@N` cursor substitution.

use super::VarSeg;
use super::repeat::{MAX_REPEAT_TOKENS, RepeatCtx, expand_repeat_blocks};
use proc_macro2::TokenStream;
use std::cell::Cell;

fn segs() -> Vec<VarSeg> {
    vec![
        VarSeg { prefix: "A".into(), start: 0, len: 3 },
        VarSeg { prefix: "B".into(), start: 1, len: 2 },
    ]
}

/// A mapping binding each segment element `(prefix, pos)` to a readable
/// stand-in token stream (`TA0`, `TB1`, ...) — the value `substitute`
/// splices into each round.
fn mapping(segs: &[VarSeg]) -> super::Mapping {
    let mut m = super::Mapping::default();
    for s in segs {
        for k in 0..s.len {
            let pos = s.start + k;
            let name = format!("{}{}", s.prefix, pos);
            m.bind_seg(&s.prefix, pos, format!("T{name}").parse::<TokenStream>().unwrap()).unwrap();
        }
    }
    m
}

fn expand(s: &str) -> Result<String, String> {
    let ss = segs();
    expand_with(s, &ss)
}

fn expand_with(s: &str, segs: &[VarSeg]) -> Result<String, String> {
    expand_budget(s, segs, MAX_REPEAT_TOKENS)
}

fn expand_budget(s: &str, segs: &[VarSeg], budget: usize) -> Result<String, String> {
    let ts = s.parse::<TokenStream>().map_err(|e| e.to_string())?;
    let cx = RepeatCtx {
        segs,
        map: &mapping(segs),
        fresh: &crate::codegen::FreshCtx::new(&[], &Default::default()),
        binding: None,
        budget: Cell::new(budget),
    };
    expand_repeat_blocks(ts, &cx).map(|o| o.to_string()).map_err(|e| e.to_string())
}

/// A cursor-only block with **no** template segment: the fresh count drives
/// the rounds (`@N` → `N + i` per round).
fn expand_fresh(s: &str, n: usize) -> Result<String, String> {
    expand_fresh_from(s, 0, n)
}

/// [`expand_fresh`] with a chosen binding start (`impl{@start..}`).
fn expand_fresh_from(s: &str, start: usize, n: usize) -> Result<String, String> {
    let ts = s.parse::<TokenStream>().map_err(|e| e.to_string())?;
    let cx = RepeatCtx {
        segs: &[],
        map: &Default::default(),
        fresh: &crate::codegen::FreshCtx::new(&fresh_names(n), &Default::default()),
        binding: Some(crate::ast::fresh::FreshRef {
            group: None,
            start,
            end: crate::ast::fresh::FreshEnd::Open,
        }),
        budget: Cell::new(MAX_REPEAT_TOKENS),
    };
    expand_repeat_blocks(ts, &cx).map(|o| o.to_string()).map_err(|e| e.to_string())
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
    // `@{N}` → the N-th fresh generic's name, **fixed** (not per-round):
    // `@{1}` names the second fresh in every round
    assert_eq!(expand_fresh("@(@{1},)..", 3).unwrap(), "P1 , P1 , P1 ,");
}

#[test]
fn fresh_name_cursor_reference() {
    // `@{@N}` → the (N + round)-th fresh's name, **per-round**: each round
    // names its own fresh — the user's `(@(@{@N}::foo(),)..)` spelling
    // emits `(P0::foo(), P1::foo(), P2::foo())` on three freshs
    assert_eq!(
        expand_fresh("@(@{@0}::foo(),)..", 3).unwrap(),
        "P0 :: foo () , P1 :: foo () , P2 :: foo () ,"
    );
}

#[test]
fn fresh_name_cursor_offset_start() {
    // the cursor is relative to the binding: with `impl{@1..}` (2 bound
    // freshs), `@{@1}` names fresh 1 then 2 — never crossing the end
    assert_eq!(expand_fresh_from("@(@{@1},)..", 1, 3).unwrap(), "P1 , P2 ,");
}

#[test]
fn fresh_name_cursor_out_of_range() {
    // round 1's `@{@2}` → `@{3}` past the end (3 freshs)
    assert!(expand_fresh("@(@{@2},)..", 3).is_err());
}

#[test]
fn fresh_name_cursor_bad_inner() {
    // `@{@x}` — the cursor must be a number
    assert!(expand_fresh("@(@{@x},)..", 3).is_err());
}

#[test]
fn fresh_name_out_of_range() {
    assert!(expand_fresh("@(@{5},)..", 3).is_err());
}

#[test]
fn no_switch_no_segment_errors() {
    // without the fresh-binding switch (`impl{@0..}`), a cursor-only block
    // with no template segment errors — fresh-driven body modification is off
    let ts = "@(args.@0,)..".parse::<TokenStream>().unwrap();
    let cx = RepeatCtx {
        segs: &[],
        map: &Default::default(),
        fresh: &crate::codegen::FreshCtx::new(&[], &Default::default()),
        binding: None,
        budget: Cell::new(MAX_REPEAT_TOKENS),
    };
    assert!(expand_repeat_blocks(ts, &cx).is_err());
}

#[test]
fn single_segment_rounds() {
    // `@A` splices the segment's i-th **bound element** directly — the
    // round's output shows the actual value, no intermediate spelling
    assert_eq!(
        expand("@(@A::f(&self.@0),)..").unwrap(),
        "TA0 :: f (& self .0) , TA1 :: f (& self .1) , TA2 :: f (& self .2) ,"
    );
}

#[test]
fn offset_start_name_numbering() {
    // B starts at leaf index 1: elements B1, B2; `@1` cursor → 1, 2.
    let segs = vec![VarSeg { prefix: "B".into(), start: 1, len: 2 }];
    assert_eq!(
        expand_with("@(@B::f(&self.@1),)..", &segs).unwrap(),
        "TB1 :: f (& self .1) , TB2 :: f (& self .2) ,"
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
    assert_eq!(expand_with("@(@A + @B,)..", &segs).unwrap(), "TA0 + TB2 , TA1 + TB3 ,");
}

#[test]
fn unequal_segment_lengths_error() {
    let segs = vec![
        VarSeg { prefix: "A".into(), start: 0, len: 3 },
        VarSeg { prefix: "B".into(), start: 1, len: 2 },
    ];
    let ts = "@(@A + @B,)..".parse::<TokenStream>().unwrap();
    let cx = RepeatCtx {
        segs: &segs,
        map: &mapping(&segs),
        fresh: &crate::codegen::FreshCtx::new(&[], &Default::default()),
        binding: None,
        budget: Cell::new(MAX_REPEAT_TOKENS),
    };
    assert!(expand_repeat_blocks(ts, &cx).is_err());
}

#[test]
fn nested_cartesian() {
    // Outer rounds A0/A1/A2; each inner runs B over B1,B2. The outer
    // block body has no trailing comma (the inner block's own trailing
    // commas separate the B elements), so no double comma appears. The
    // inner expansion splices its values directly — no carrier passes
    // through the outer substitution.
    let out = expand("@(@A::f(&self.@0) @(@B::g(&self.@1),)..)..").unwrap();
    assert_eq!(
        out,
        "TA0 :: f (& self .0) TB1 :: g (& self .1) , TB2 :: g (& self .2) , \
         TA1 :: f (& self .1) TB1 :: g (& self .1) , TB2 :: g (& self .2) , \
         TA2 :: f (& self .2) TB1 :: g (& self .1) , TB2 :: g (& self .2) ,"
    );
}

#[test]
fn no_trailing_separator_concatenates() {
    assert_eq!(expand("@(@A)..").unwrap(), "TA0 TA1 TA2");
}

#[test]
fn inter_round_separator() {
    // `@(@A),..` — the comma sits between rounds, never after the last one
    // (the `$($A),*` form; `@(@A,)..` is the `$($A,)*` form)
    assert_eq!(expand("@(@A),..").unwrap(), "TA0 ,TA1 ,TA2");
}

#[test]
fn inter_round_separator_arbitrary() {
    // any literal tokens work as the inter-round separator
    assert_eq!(expand("@(@A)+..").unwrap(), "TA0 +TA1 +TA2");
    assert_eq!(expand("@(@A)::f()..").unwrap(), "TA0 :: f () TA1 :: f () TA2");
}

#[test]
fn inter_round_separator_single_round() {
    // one round: no separator is emitted at all
    let segs = vec![VarSeg { prefix: "A".into(), start: 0, len: 1 }];
    assert_eq!(expand_with("@(@A),..", &segs).unwrap(), "TA0");
}

#[test]
fn multi_element_value_splices_whole() {
    // a bound element may be a composite type (`Vec<TA0>`); the splice
    // emits the whole subtree verbatim
    let segs = vec![VarSeg { prefix: "A".into(), start: 0, len: 2 }];
    let mut m = super::Mapping::default();
    m.bind_seg("A", 0, "Vec < u8 >".parse().unwrap()).unwrap();
    m.bind_seg("A", 1, "Vec < u16 >".parse().unwrap()).unwrap();
    let ts = "@(push::< @A > (),)..".parse::<TokenStream>().unwrap();
    let cx = RepeatCtx {
        segs: &segs,
        map: &m,
        fresh: &crate::codegen::FreshCtx::new(&[], &Default::default()),
        binding: None,
        budget: Cell::new(MAX_REPEAT_TOKENS),
    };
    let out = expand_repeat_blocks(ts, &cx).unwrap().to_string();
    assert_eq!(out, "push ::< Vec < u8 > > () , push ::< Vec < u16 > > () ,");
}

#[test]
fn float_literal_at_path_fixed() {
    // `self.0.@0` tokenizes `0.` as a float literal; the fix splits it
    // so the cursor expands into `self.0.0`, `self.0.1`, ...
    let segs = vec![VarSeg { prefix: "A".into(), start: 0, len: 2 }];
    let ts = "@(@A::from(self.0.@0),)..".parse::<TokenStream>().unwrap();
    let cx = RepeatCtx {
        segs: &segs,
        map: &mapping(&segs),
        fresh: &crate::codegen::FreshCtx::new(&[], &Default::default()),
        binding: None,
        budget: Cell::new(MAX_REPEAT_TOKENS),
    };
    let out = expand_repeat_blocks(ts, &cx).unwrap().to_string();
    assert_eq!(out, "TA0 :: from (self . 0 . 0) , TA1 :: from (self . 0 . 1) ,");
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
    assert_eq!(expand_with("@A(self.@0,)..", &segs).unwrap(), "self .0 , self .1 , self .2 ,");
}

#[test]
fn cursor_only_single_segment() {
    // no declared driver and no inner `@ident`: the template's unique
    // segment provides the length
    let segs = vec![VarSeg { prefix: "A".into(), start: 0, len: 2 }];
    assert_eq!(expand_with("@(self.@0,)..", &segs).unwrap(), "self .0 , self .1 ,");
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
    let cx = RepeatCtx {
        segs: &segs,
        map: &mapping(&segs),
        fresh: &crate::codegen::FreshCtx::new(&[], &Default::default()),
        binding: None,
        budget: Cell::new(MAX_REPEAT_TOKENS),
    };
    assert!(expand_repeat_blocks(ts, &cx).is_err());
}

#[test]
fn declared_driver_conflict_errors() {
    let segs = vec![
        VarSeg { prefix: "A".into(), start: 0, len: 2 },
        VarSeg { prefix: "B".into(), start: 2, len: 2 },
    ];
    let ts = "@A(@B::f(),)..".parse::<TokenStream>().unwrap();
    let cx = RepeatCtx {
        segs: &segs,
        map: &mapping(&segs),
        fresh: &crate::codegen::FreshCtx::new(&[], &Default::default()),
        binding: None,
        budget: Cell::new(MAX_REPEAT_TOKENS),
    };
    assert!(expand_repeat_blocks(ts, &cx).is_err());
}

#[test]
fn nested_output_exceeds_budget_errors() {
    // Cartesian semantics multiply the output (∏len over nesting levels);
    // three len-40 levels emit ~64k tokens — over the budget, a targeted
    // diagnostic instead of unbounded emission.
    let segs = vec![VarSeg { prefix: "A".into(), start: 0, len: 40 }];
    let ts = "@(@(@(@A,)..)..)..".parse::<TokenStream>().unwrap();
    let out = expand_budget(&ts.to_string(), &segs, MAX_REPEAT_TOKENS);
    assert!(out.unwrap_err().contains("limit 65536"));
}

#[test]
fn nested_output_under_budget_expands() {
    // the same shape, one level fewer (1600 rounds): well under budget
    let segs = vec![VarSeg { prefix: "A".into(), start: 0, len: 40 }];
    let out = expand_budget("@(@(@A,)..)..", &segs, MAX_REPEAT_TOKENS).unwrap();
    assert_eq!(out.matches("TA").count(), 40 * 40);
}

#[test]
fn tiny_budget_rejects_even_single_rounds() {
    // the budget is absolute output tokens, not round count
    let segs = vec![VarSeg { prefix: "A".into(), start: 0, len: 3 }];
    assert!(expand_budget("@(@A),..", &segs, 2).is_err());
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
