//! Continuation, host and resource-bound regression cases for Pack integration.

use super::pack::{map_task, packify};
use super::pack_tests::*;
use crate::ast::*;
use crate::util::MAX_NEST_DEPTH;
use quote::{ToTokens, quote};

#[test]
fn right_errors_survive_empty_packs_without_being_duplicated() {
    let error = super::err_ty("sentinel");
    for left in [pack(vec![]), pack(vec![atom("F"), atom("G")])] {
        let actual = map_task(left, error.clone());
        assert!(matches!(actual.kind, TyKind::Error(_)));
        same(&actual, &error);
    }
}

#[test]
fn right_metadata_is_lifted_before_selecting_a_shared_candidate() {
    let constructors = pack(vec![atom("F"), atom("G")]);
    let candidates = choice(vec![atom("A"), atom("B")]);
    let expected = choice(vec![
        pack(vec![atom("F").apply(atom("A")), atom("G").apply(atom("A"))]),
        pack(vec![atom("F").apply(atom("B")), atom("G").apply(atom("B"))]),
    ]);
    let body = TyCodeBlock(quote!(
        fn f() {}
    ));
    same(
        &map_task(
            constructors.clone(),
            TyWithCode(candidates.clone().into(), body.clone()).to_ty(),
        ),
        &TyWithCode(expected.clone().into(), body).to_ty(),
    );
    let clause = TyWhere(quote!(T: Clone));
    same(
        &map_task(
            constructors.clone(),
            TyWithWhere(candidates.clone().into(), clause.clone()).to_ty(),
        ),
        &TyWithWhere(expected.clone().into(), clause).to_ty(),
    );
    let template = TyImplTemplate(quote!(T));
    let mapped = map_task(constructors, TyWithImpl(candidates.into(), template.clone()).to_ty());
    let TyKind::WithImpl(actual) = &mapped.kind else {
        panic!("template must surround all branches")
    };
    same(actual.0.as_deref().unwrap(), &expected);
    assert_eq!(actual.1.0.to_string(), template.0.to_string());
}

#[test]
fn packify_treats_complete_rust_type_hosts_as_single_items() {
    let nested = pack(vec![atom("A"), atom("B")]);
    let hosts = [
        TyWithPrefix(TyPrefix::Ref, nested.clone().into()).to_ty(),
        TyWithPrefix(TyPrefix::PtrConst, nested.clone().into()).to_ty(),
        TyPrimitiveArray(nested.clone().into(), None).to_ty(),
        atom("F").apply(nested.clone()),
        TyFn(Some(vec![nested]), None, false, FnKind::Bare).to_ty(),
    ];
    for host in hosts {
        same(&packify(host.clone()), &pack(vec![host]));
    }
}

#[test]
fn pair_mapping_keeps_each_shared_fresh_argument_pack_intact() {
    reset_fresh_counter();
    let input = generated(3);
    let rows = pack(vec![identity(), atom("Vec")]).apply(input.clone());
    let result = pack(vec![atom("Pair")]).apply(rows);
    let (params, body) = split_decl(&result);
    assert_eq!(params.params.len(), 3);
    let expected = split_decl(&input)
        .0
        .params
        .iter()
        .map(|(fresh, _)| {
            let args =
                pack(vec![fresh.as_ref().clone(), atom("Vec").apply(fresh.as_ref().clone())]);
            pack(vec![atom("Pair").apply(args)])
        })
        .collect();
    same(body, &pack(expected));
}

#[test]
fn direct_ast_inputs_are_checked_for_depth_and_mass() {
    let deep = (0..MAX_NEST_DEPTH + 2).fold(atom("F"), |inner, _| TyGroup(inner.into()).to_ty());
    for error in [packify(deep.clone()), map_task(deep, atom("A"))] {
        assert!(matches!(error.kind, TyKind::Error(_)));
        assert!(error.to_token_stream().to_string().contains("nesting depth"));
    }
    let too_wide = pack(vec![atom("F"); MAX_EXPAND + 1]);
    assert!(matches!(packify(too_wide.clone()).kind, TyKind::Error(_)));
    assert!(matches!(map_task(too_wide, atom("A")).kind, TyKind::Error(_)));
}

#[test]
fn oversized_numeric_generation_is_rejected_before_allocating_fresh() {
    reset_fresh_counter();
    let bound = TyTypeParam::single(&atom("Clone")).to_ty();
    let error = pack(vec![bound]).apply(TyNum(MAX_EXPAND).to_ty());
    assert!(matches!(error.kind, TyKind::Error(_)));
    let next = generated(1);
    let TyKind::Fresh(TyFresh(reference)) = split_decl(&next).0.params[0].0.kind else {
        panic!("expected fresh")
    };
    assert_eq!(reference.group, Some(0));
    let large = tuple(vec![atom("A"); MAX_EXPAND / 2]);
    assert!(matches!(pack(vec![large]).apply(TyNum(3).to_ty()).kind, TyKind::Error(_)));
}

#[test]
fn merged_declarations_and_new_pack_shells_obey_the_result_limit() {
    let wide = tuple(vec![atom("A"); MAX_EXPAND - 2]);
    // Input fits exactly, but wrapping a non-container needs another node.
    let host = TyWithPrefix(TyPrefix::Ref, wide.into()).to_ty();
    assert!(matches!(packify(host).kind, TyKind::Error(_)));
    let carrier = |inner: Ty| {
        TyWithType(
            TyTypeParam {
                params: vec![(atom("T").into(), None); MAX_EXPAND / 2],
                bindings: vec![],
            },
            inner.into(),
        )
        .to_ty()
    };
    assert!(matches!(
        map_task(carrier(pack(vec![identity()])), carrier(atom("A"))).kind,
        TyKind::Error(_)
    ));
}

#[test]
fn generation_at_the_exact_mass_limit_counts_merged_carriers_once() {
    for (slots, length) in [
        (vec![], (MAX_EXPAND - 4) / 2),
        (vec![TyTypeParam::single(&atom("Clone")).to_ty()], (MAX_EXPAND - 4) / 3),
    ] {
        reset_fresh_counter();
        let carrier = TyWithType(
            TyTypeParam {
                params: vec![(atom("T").into(), None), (atom("U").into(), None)],
                bindings: vec![],
            },
            pack(slots).into(),
        )
        .to_ty();
        let actual = pack(vec![carrier]).apply(TyNum(length).to_ty());
        assert!(matches!(actual.kind, TyKind::WithType(_)), "{actual:?}");
        assert_eq!(count_leaves(&actual), MAX_EXPAND);
    }
}
