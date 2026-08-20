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
    expand_repeat_blocks(ts, &segs()).map(|o| o.to_string()).map_err(|e| e.to_string())
}

#[test]
fn single_segment_rounds() {
    assert_eq!(
        expand("@(@A::f(&self.@0),)..").unwrap(),
        "A0 :: f (& self .0) , A1 :: f (& self .1) , A2 :: f (& self .2) ,"
    );
}

#[test]
fn offset_start_name_numbering() {
    // B starts at leaf index 1: names B1, B2; `@1` cursor → 1, 2.
    assert_eq!(
        expand("@(@B::f(&self.@1),)..").unwrap(),
        "B1 :: f (& self .1) , B2 :: f (& self .2) ,"
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
    let out = expand_repeat_blocks(ts, &segs).unwrap().to_string();
    assert_eq!(out, "A0 + B2 , A1 + B3 ,");
}

#[test]
fn unequal_segment_lengths_error() {
    let segs = vec![
        VarSeg { prefix: "A".into(), start: 0, len: 3 },
        VarSeg { prefix: "B".into(), start: 1, len: 2 },
    ];
    let ts = "@(@A + @B,)..".parse::<TokenStream>().unwrap();
    assert!(expand_repeat_blocks(ts, &segs).is_err());
}

#[test]
fn nested_cartesian() {
    // Outer rounds A0/A1/A2; each inner runs B over 1,2. The outer
    // block body has no trailing comma (the inner block's own trailing
    // commas separate the B elements), so no double comma appears.
    let out = expand("@(@A::f(&self.@0) @(@B::g(&self.@1),)..)..").unwrap();
    assert_eq!(
        out,
        "A0 :: f (& self .0) B1 :: g (& self .1) , B2 :: g (& self .2) , \
         A1 :: f (& self .1) B1 :: g (& self .1) , B2 :: g (& self .2) , \
         A2 :: f (& self .2) B1 :: g (& self .1) , B2 :: g (& self .2) ,"
    );
}

#[test]
fn no_trailing_separator_concatenates() {
    assert_eq!(expand("@(@A)..").unwrap(), "A0 A1 A2");
}

#[test]
fn float_literal_at_path_fixed() {
    // `self.0.@0` tokenizes `0.` as a float literal; the fix splits it
    // so the cursor expands into `self.0.0`, `self.0.1`, ...
    let segs = vec![VarSeg { prefix: "A".into(), start: 0, len: 2 }];
    let ts = "@(@A::from(self.0.@0),)..".parse::<TokenStream>().unwrap();
    let out = expand_repeat_blocks(ts, &segs).unwrap().to_string();
    assert_eq!(out, "A0 :: from (self . 0 . 0) , A1 :: from (self . 0 . 1) ,");
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
    let out = expand_repeat_blocks(ts, &segs).unwrap().to_string();
    assert_eq!(out, "self .0 , self .1 , self .2 ,");
}

#[test]
fn cursor_only_single_segment() {
    // no declared driver and no inner `@ident`: the template's unique
    // segment provides the length
    let segs = vec![VarSeg { prefix: "A".into(), start: 0, len: 2 }];
    let ts = "@(self.@0,)..".parse::<TokenStream>().unwrap();
    let out = expand_repeat_blocks(ts, &segs).unwrap().to_string();
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
    assert!(expand_repeat_blocks(ts, &segs).is_err());
}

#[test]
fn declared_driver_conflict_errors() {
    let segs = vec![
        VarSeg { prefix: "A".into(), start: 0, len: 2 },
        VarSeg { prefix: "B".into(), start: 2, len: 2 },
    ];
    let ts = "@A(@B::f(),)..".parse::<TokenStream>().unwrap();
    assert!(expand_repeat_blocks(ts, &segs).is_err());
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
fn no_driver_errors() {
    assert!(expand("@(@0,)..").is_err());
}
