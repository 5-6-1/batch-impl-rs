use super::repeat::{MAX_REPEAT_TOKENS, RepeatCtx, expand_repeat_blocks};
use crate::codegen::{FreshCtx, Mapping, VarSeg};
use proc_macro2::TokenStream;
use std::cell::Cell;

fn expand(
    source: &str, len: usize, element: &str, limit: usize,
) -> Result<(String, usize), String> {
    let rows = (0..len)
        .map(|_| {
            let mut row = Mapping::default();
            row.bind("A", element.parse::<TokenStream>().unwrap()).unwrap();
            row
        })
        .collect();
    let segs = [VarSeg { names: vec!["A".into()], start: 0, rows }];
    let map = Mapping::default();
    let fresh = FreshCtx::new(&[], &Default::default());
    let cx = RepeatCtx {
        domain: crate::util::TokenDomain::BodySubstitution,
        segs: &segs,
        map: &map,
        fresh: &fresh,
        binding: None,
        budget: Cell::new(limit),
    };
    expand_repeat_blocks(source.parse().unwrap(), &cx)
        .map(|tokens| (tokens.to_string(), cx.budget.get()))
        .map_err(|tokens| tokens.to_string())
}

#[test]
fn groups_cannot_hide_cartesian_repeat_output() {
    let error = expand("@(@(@({n+=1;})..)..)..", 32, "u8", MAX_REPEAT_TOKENS).unwrap_err();
    assert!(error.contains("limit 65536"), "{error}");
}

#[test]
fn nested_rounds_count_final_output_once_at_the_exact_limit() {
    // 32^3 rounds * two tokens (group and x) = exactly 65536.
    let source = "@(@(@({x})..)..)..";
    let (tokens, remaining) = expand(source, 32, "u8", MAX_REPEAT_TOKENS).unwrap();
    assert_eq!(tokens.matches('x').count(), 32768);
    assert_eq!(remaining, 0);
    assert!(expand(source, 32, "u8", MAX_REPEAT_TOKENS - 1).is_err());
    assert_eq!(expand("@({x})..", 0, "u8", 0).unwrap(), (String::new(), 0));
}

#[test]
fn ordinary_groups_and_separators_share_the_body_budget() {
    // outer group + two five-token inner groups + the separator = 12 tokens.
    let source = "{@({n+=1}) , ..}";
    assert_eq!(expand(source, 2, "u8", 12).unwrap().1, 0);
    assert!(expand(source, 2, "u8", 11).is_err());
    assert_eq!(expand("{{x}}", 1, "u8", 3).unwrap().1, 0);
    assert!(expand("{{x}}", 1, "u8", 2).is_err());
}

#[test]
fn substitutions_count_all_bound_tokens_before_copying_more() {
    assert_eq!(expand("@(@A)..", 2, "(u8,u16)", 8).unwrap().1, 0);
    assert!(expand("@(@A)..", 2, "(u8,u16)", 7).is_err());
    // Two source tokens shrink to one output token; source is not the budget.
    assert_eq!(expand("@(@A)..", 1, "u8", 1).unwrap().1, 0);
}

#[test]
fn plain_source_and_separate_blocks_cannot_bypass_the_limit() {
    let source = "x ".repeat(MAX_REPEAT_TOKENS);
    assert_eq!(expand(&source, 1, "u8", MAX_REPEAT_TOKENS).unwrap().1, 0);
    assert!(expand(&(source + "x"), 1, "u8", MAX_REPEAT_TOKENS).is_err());
    assert_eq!(expand("@(@A).. @(@A)..", 2, "u8", 4).unwrap().1, 0);
    assert!(expand("@(@A).. @(@A)..", 2, "u8", 3).is_err());
}

#[test]
fn enabled_macro_inputs_and_attributes_are_rewritten_and_counted() {
    for source in ["stringify!(@(@A)..)", "m![@(@A)..]", "m!{@(@A)..}", "#[attr(@(@A)..)] x"] {
        let direct = expand(source, 2, "u8", MAX_REPEAT_TOKENS).unwrap().0;
        assert_eq!(direct.matches("u8").count(), 2);
        let repeated = expand(&format!("@A({source}).."), 2, "u8", MAX_REPEAT_TOKENS).unwrap().0;
        assert_eq!(repeated.matches("u8").count(), 4);
    }
    // m, !, group, u8, u8: macro contents count toward the same budget.
    assert_eq!(expand("m!(@(@A)..)", 2, "u8", 5).unwrap().1, 0);
    assert!(expand("m!(@(@A)..)", 2, "u8", 4).is_err());
    let nested = expand("@A(@A(stringify!(@A))..)..", 2, "u8", 16).unwrap();
    assert_eq!(nested.0.matches("u8").count(), 4);
    assert_eq!(nested.1, 0);
}

#[test]
fn empty_fresh_ranges_charge_folded_tuple_output() {
    for (source, count, expected) in [
        ("(@{0..},)", 1, "()"),
        ("(@{0..}, x,)", 3, "(x,)"),
        ("(@{0..}, (u8, u16),)", 6, "((u8,u16),)"),
        ("@A((@{0..},))..", 2, "()()"),
        ("@A(@A((@{0..},))..)..", 4, "()()()()"),
    ] {
        let (tokens, remaining) = expand(source, 2, "u8", count).unwrap();
        assert_eq!(tokens.replace(' ', ""), expected, "{source}");
        assert_eq!(remaining, 0, "{source}");
        assert!(expand(source, 2, "u8", count - 1).is_err(), "{source}");
    }
}

#[test]
fn temporary_comma_allowance_cannot_enlarge_final_output() {
    // Folding rejoins the two surviving elements without a trailing comma;
    // both elements, their separator, and the enclosing group still count.
    let source = "(@{0..}, x, y,)";
    assert_eq!(expand(source, 1, "u8", 4).unwrap().1, 0);
    assert!(expand(source, 1, "u8", 3).is_err());
    // Macro inputs participate in an enabled body, with the same final count.
    assert_eq!(expand("m!((@{0..},))", 1, "u8", 4).unwrap().1, 0);
    assert!(expand("m!((@{0..},))", 1, "u8", 3).is_err());
    // A captured group that resembles an unresolved range stays literal.
    let (tokens, remaining) = expand("@A(@A)..", 1, "(@{0..},)", 7).unwrap();
    assert_eq!(tokens.replace(' ', ""), "(@{0..},)");
    assert_eq!(remaining, 0);
    assert!(expand("@A(@A)..", 1, "(@{0..},)", 6).is_err());
}
