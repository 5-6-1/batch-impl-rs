use super::pack_tests::{atom, choice, pack, tuple};
use crate::ast::*;
use crate::codegen::{FreshCtx, hoist_type_params};
use quote::{ToTokens, quote};

fn range(start: usize, end: usize) -> Ty {
    TyRange { start, end, inclusive: false }.to_ty()
}

fn contexts(value: Ty) -> Vec<FreshCtx> {
    materialize_targets(value)
        .unwrap()
        .into_iter()
        .map(|leaf| {
            let mut declarations = vec![];
            hoist_type_params(leaf, &mut declarations);
            FreshCtx::new(
                &declarations.into_iter().map(|(name, _)| name).collect::<Vec<_>>(),
                &Default::default(),
            )
        })
        .collect()
}

#[test]
fn range_sources_do_not_follow_cartesian_allocation_counts() {
    reset_fresh_counter();
    let sources = choice(vec![tuple(vec![atom("u8"), atom("u16")]), tuple(vec![])]);
    let contexts = contexts(sources.apply(range(1, 3)));
    assert_eq!(contexts.len(), 8);
    let generated = contexts.iter().filter(|ctx| ctx.has_group(0)).collect::<Vec<_>>();
    assert_eq!(generated.len(), 2);
    assert_eq!(generated.iter().map(|ctx| ctx.names.len()).collect::<Vec<_>>(), vec![1, 2]);
    assert_eq!(take_group(), 1);
}

#[test]
fn each_choice_source_has_its_own_stable_group() {
    reset_fresh_counter();
    let sources = choice(vec![tuple(vec![]), tuple(vec![])]);
    let contexts = contexts(sources.apply(range(0, 3)));
    for (index, ctx) in contexts.iter().enumerate() {
        assert!(ctx.has_group(index % 2));
        assert!(!ctx.has_group(1 - index % 2));
        assert_eq!(ctx.names.len(), index / 2);
    }
    assert_eq!(take_group(), 2);
}

#[test]
fn nested_ranges_preserve_existing_sources_and_identify_new_ones() {
    reset_fresh_counter();
    let inner = tuple(vec![]).apply(range(0, 2));
    let contexts = contexts(inner.apply(range(1, 3)));
    assert_eq!(contexts.len(), 4);
    for (index, ctx) in contexts.iter().enumerate() {
        assert!(ctx.has_group(0));
        assert_eq!(ctx.has_group(1), index % 2 == 0);
        assert_eq!(ctx.names.len(), if index % 2 == 0 { index / 2 + 1 } else { 1 });
    }
    assert_eq!(take_group(), 2);
}

#[test]
fn groups_are_branch_local_and_known_empty_is_not_unknown() {
    reset_fresh_counter();
    let generated = pack(vec![]).apply(range(0, 1));
    let contexts = contexts(choice(vec![tuple(vec![generated]), atom("u8")]));
    assert!(contexts[0].group(0, proc_macro2::Span::call_site()).unwrap().is_empty());
    assert!(contexts[1].group(0, proc_macro2::Span::call_site()).is_err());
    let ctx = &contexts[0];
    assert!(crate::codegen::expand_range_refs(quote!((@{0_0..},)), ctx).is_ok());
    for input in [quote!(@{0_0}), quote!(@{0_0..=0}), quote!(@{1_0..})] {
        assert!(crate::codegen::expand_range_refs(input, ctx).is_err());
    }
}

#[test]
fn empty_group_protocol_only_parses_as_a_declaration() {
    for spec in [quote!(Vec<@{group 0}>), quote!((@{group 0},))] {
        let result = crate::entry::expand_attr_macro(
            spec,
            syn::parse_quote!(
                trait Marker {}
            ),
            false,
        );
        assert!(result.is_err());
    }
    let output = crate::entry::expand_attr_macro(
        quote!(<@{group 0}> () where { @0_0..: Clone }),
        syn::parse_quote!(
            trait Marker {}
        ),
        false,
    )
    .unwrap();
    assert_eq!(syn::parse2::<syn::ItemImpl>(output).unwrap().generics.params.len(), 0);
}

#[test]
fn failed_ranges_do_not_leak_a_replay_context_to_the_next_spec() {
    reset_fresh_counter();
    let error = tuple(vec![]).apply(range(2, 1));
    assert!(matches!(error.kind, TyKind::Error(_)));
    assert_eq!(take_group(), 0);
    reset_fresh_counter();
    let value = tuple(vec![]).apply(TyNum(1).to_ty());
    assert!(value.to_token_stream().to_string().contains("0_0"));
}
