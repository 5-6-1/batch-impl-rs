use super::repeat::{MAX_REPEAT_TOKENS, RepeatCtx, expand_repeat_blocks};
use crate::codegen::{FreshCtx, Mapping, VarSeg};
use proc_macro2::TokenStream;
use std::cell::Cell;

fn expand(
    source: &str, len: usize, element: &str, limit: usize,
) -> Result<(String, usize), String> {
    let segs = [VarSeg { prefix: "A".into(), start: 0, len }];
    let mut map = Mapping::default();
    for index in 0..len {
        map.bind_seg("A", index, element.parse::<TokenStream>().unwrap()).unwrap();
    }
    let fresh = FreshCtx::new(&[], &Default::default());
    let cx = RepeatCtx {
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
fn macro_inputs_and_attributes_are_opaque_but_still_counted() {
    for source in ["stringify!(@unknown)", "m![@unknown]", "m!{@unknown}", "#[attr(@unknown)] x"] {
        let direct = expand(source, 2, "u8", MAX_REPEAT_TOKENS).unwrap().0;
        assert_eq!(direct, source.parse::<TokenStream>().unwrap().to_string());
        let repeated = expand(&format!("@A({source}).."), 2, "u8", MAX_REPEAT_TOKENS).unwrap().0;
        assert_eq!(repeated.matches("unknown").count(), 2);
    }
    // m, !, group, @, unknown: an opaque group still contains three tokens.
    assert_eq!(expand("m!(@unknown)", 1, "u8", 5).unwrap().1, 0);
    assert!(expand("m!(@unknown)", 1, "u8", 4).is_err());
    let nested = expand("@A(@A(stringify!(@unknown))..)..", 2, "u8", 20).unwrap();
    assert_eq!(nested.0.matches("unknown").count(), 4);
    assert_eq!(nested.1, 0);
}
