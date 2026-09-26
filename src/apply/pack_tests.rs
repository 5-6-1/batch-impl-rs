//! Structural assertions for the internal kernel. No test uses the legacy
//! splat collector as a substitute for future Pack materialization.

use super::pack::{map_task, packify};
use crate::ast::*;
use quote::{ToTokens, quote};

pub(super) fn atom(name: &str) -> Ty {
    TyPrimitive(name.parse().unwrap()).to_ty()
}

pub(super) fn pack(items: Vec<Ty>) -> Ty {
    TyPack(items).to_ty()
}

pub(super) fn choice(items: Vec<Ty>) -> Ty {
    TyArray(items).to_ty()
}

pub(super) fn tuple(items: Vec<Ty>) -> Ty {
    TyTuple(items).to_ty()
}

pub(super) fn identity() -> Ty {
    TyWithPrefix(TyPrefix::SelfType, None).to_ty()
}

fn decl(name: &str, bound: Option<Ty>, inner: Ty) -> Ty {
    TyWithType(
        TyTypeParam { params: vec![(atom(name).into(), bound.map(Into::into))], bindings: vec![] },
        inner.into(),
    )
    .to_ty()
}

pub(super) fn same(actual: &Ty, expected: &Ty) {
    // Rendering preserves each raw Pack layer and declaration payload; a
    // materialized-type comparison would conceal the row-reopening bug.
    assert_eq!(actual.to_token_stream().to_string(), expected.to_token_stream().to_string());
}

pub(super) fn generated(n: usize) -> Ty {
    pack(vec![]).apply(TyNum(n).to_ty())
}

pub(super) fn split_decl(value: &Ty) -> (&TyTypeParam, &Ty) {
    let TyKind::WithType(w) = &value.kind else { panic!("expected declaration: {value:?}") };
    (&w.0, &w.1)
}

#[test]
fn packify_opens_one_layer_and_is_idempotent() {
    let nested = tuple(vec![tuple(vec![atom("A"), atom("B")]), choice(vec![atom("C"), atom("D")])]);
    let opened = packify(nested);
    same(
        &opened,
        &pack(vec![tuple(vec![atom("A"), atom("B")]), choice(vec![atom("C"), atom("D")])]),
    );
    same(&packify(opened.clone()), &opened);
    same(&packify(atom("A")), &pack(vec![atom("A")]));
    same(
        &packify(TyGroup(choice(vec![atom("A"), atom("B")]).into()).to_ty()),
        &pack(vec![atom("A"), atom("B")]),
    );
}

#[test]
fn pack_serialization_retains_singleton_layers() {
    assert_eq!(
        pack(vec![pack(vec![atom("A"), atom("B")])]).to_token_stream().to_string(),
        quote!(*(*(A, B,),)).to_string()
    );
    assert_eq!(pack(vec![atom("A")]).to_token_stream().to_string(), quote!(*(A,)).to_string());
}

#[test]
fn packify_keeps_unused_declarations_bounds_and_bindings() {
    let params = TyTypeParam {
        params: vec![(atom("T").into(), Some(atom("Clone").into()))],
        bindings: vec![(atom("Item").into(), atom("u8").into())],
    };
    let actual = packify(TyWithType(params.clone(), tuple(vec![]).into()).to_ty());
    same(&actual, &TyWithType(params, pack(vec![]).into()).to_ty());
}

#[test]
fn pairing_opens_only_direct_right_rows() {
    let row = pack(vec![atom("A"), atom("B")]);
    let actual = pack(vec![identity(), atom("Vec")]).apply(pack(vec![row.clone()]));
    same(&actual, &pack(vec![pack(vec![row.clone(), atom("Vec").apply(row)])]));
}

#[test]
fn mapping_a_choice_of_packs_keeps_the_current_row() {
    let left = pack(vec![choice(vec![pack(vec![atom("F")]), pack(vec![atom("G")])])]);
    let row = pack(vec![atom("A"), atom("B")]);
    let actual = left.apply(pack(vec![row.clone()]));
    let expected = pack(vec![pack(vec![choice(vec![
        pack(vec![atom("F").apply(row.clone())]),
        pack(vec![atom("G").apply(row)]),
    ])])]);
    same(&actual, &expected);
}

#[test]
fn transparent_left_wrappers_keep_the_current_map_task() {
    let row = pack(vec![atom("A"), atom("B")]);
    let grouped = TyGroup(pack(vec![atom("F")]).into()).to_ty();
    same(&map_task(grouped, row.clone()), &pack(vec![atom("F").apply(row.clone())]));
    let actual = map_task(decl("T", Some(atom("Clone")), pack(vec![atom("F")])), row.clone());
    same(&actual, &decl("T", Some(atom("Clone")), pack(vec![atom("F").apply(row)])));
}

#[test]
fn attached_metadata_preserves_mapping_continuation() {
    let row = pack(vec![atom("A"), atom("B")]);
    let left = TyWithCode(
        pack(vec![atom("F")]).into(),
        TyCodeBlock(quote!(
            fn f() {}
        )),
    )
    .to_ty();
    let expected = TyWithCode(
        pack(vec![atom("F").apply(row.clone())]).into(),
        TyCodeBlock(quote!(
            fn f() {}
        )),
    )
    .to_ty();
    same(&map_task(left, row), &expected);
}

#[test]
fn exposed_right_choices_are_shared_but_hidden_choices_are_copied() {
    let constructors = pack(vec![identity(), atom("Vec")]);
    let actual = constructors.clone().apply(choice(vec![atom("A"), atom("B")]));
    same(
        &actual,
        &choice(vec![
            pack(vec![atom("A"), atom("Vec").apply(atom("A"))]),
            pack(vec![atom("B"), atom("Vec").apply(atom("B"))]),
        ]),
    );
    let hidden = tuple(vec![choice(vec![atom("A"), atom("B")])]);
    same(
        &constructors.apply(hidden.clone()),
        &pack(vec![hidden.clone(), atom("Vec").apply(hidden)]),
    );
}

#[test]
fn an_ordinary_constructor_preserves_the_whole_argument_pack() {
    let args = pack(vec![atom("A"), atom("Vec").apply(atom("A"))]);
    let actual = atom("Pair").apply(args.clone());
    let TyKind::Generic(g) = actual.kind else { panic!("expected generic") };
    assert_eq!(g.1.params.len(), 1);
    same(&g.1.params[0].0, &args);
}

#[test]
fn empty_pack_and_a_pack_containing_one_empty_row_differ() {
    let f = pack(vec![atom("F")]);
    same(&f.clone().apply(pack(vec![])), &pack(vec![]));
    same(
        &f.apply(pack(vec![pack(vec![])])),
        &pack(vec![pack(vec![atom("F").apply(pack(vec![]))])]),
    );
    same(
        &pack(vec![]).apply(pack(vec![atom("A"), atom("B")])),
        &pack(vec![pack(vec![]), pack(vec![])]),
    );
}

#[test]
fn declaration_carriers_remain_left_first_without_name_deduplication() {
    let actual = decl("T", Some(atom("Clone")), pack(vec![identity()])).apply(decl(
        "T",
        Some(atom("Copy")),
        atom("A"),
    ));
    let (params, inner) = split_decl(&actual);
    assert_eq!(params.params.len(), 2);
    same(params.params[0].1.as_deref().unwrap(), &atom("Clone"));
    same(params.params[1].1.as_deref().unwrap(), &atom("Copy"));
    same(inner, &pack(vec![atom("A")]));
}

#[test]
fn numeric_pack_consumption_flattens_packs_but_keeps_candidates() {
    let candidates = choice(vec![atom("A"), atom("B")]);
    let actual = pack(vec![pack(vec![candidates.clone()])]).apply(TyNum(2).to_ty());
    same(&actual, &pack(vec![candidates.clone(), candidates]));
    let tuple_slot = tuple(vec![atom("A"), atom("B")]);
    same(
        &pack(vec![tuple_slot.clone()]).apply(TyNum(2).to_ty()),
        &pack(vec![tuple_slot.clone(), tuple_slot]),
    );
}

#[test]
fn numeric_pack_cartesian_results_stay_candidates_of_packs() {
    let actual = pack(vec![atom("A"), atom("B")]).apply(TyNum(2).to_ty());
    same(
        &actual,
        &choice(vec![
            pack(vec![atom("A"), atom("A")]),
            pack(vec![atom("A"), atom("B")]),
            pack(vec![atom("B"), atom("A")]),
            pack(vec![atom("B"), atom("B")]),
        ]),
    );
}

#[test]
fn fresh_copies_keep_identity_and_zero_generation_allocates_no_group() {
    reset_fresh_counter();
    for template in [
        pack(vec![]),
        pack(vec![TyTypeParam::single(&atom("Clone")).to_ty()]),
        pack(vec![atom("A"), atom("B")]),
    ] {
        template.apply(TyNum(0).to_ty());
    }
    let first = generated(1);
    let (params, _) = split_decl(&first);
    let TyKind::Fresh(TyFresh(reference)) = params.params[0].0.kind else {
        panic!("expected fresh")
    };
    assert_eq!(reference.group, Some(0));
    let copied = pack(vec![first.clone()]).apply(TyNum(2).to_ty());
    let (_, inner) = split_decl(&copied);
    same(
        inner,
        &pack(vec![params.params[0].0.as_ref().clone(), params.params[0].0.as_ref().clone()]),
    );
}

#[test]
fn zero_axis_keeps_prior_declarations_but_dropped_tuple_slots_do_not() {
    reset_fresh_counter();
    let left = pack(vec![atom("Map")]).apply(generated(2));
    let result = left.apply(generated(0));
    let (params, inner) = split_decl(&result);
    assert_eq!(params.params.len(), 2);
    same(inner, &pack(vec![]));
    same(&tuple(vec![generated(2)]).apply(TyNum(0).to_ty()), &tuple(vec![]));
}

#[test]
fn range_branches_generate_distinct_fresh_groups_after_zero() {
    reset_fresh_counter();
    let value = pack(vec![]).apply(TyRange { start: 0, end: 2, inclusive: true }.to_ty());
    let TyKind::Array(branches) = value.kind else { panic!("expected range branches") };
    assert_eq!(branches.0.len(), 3);
    same(&branches.0[0], &pack(vec![]));
    for (branch, expected_group) in branches.0[1..].iter().zip(0..) {
        let (params, _) = split_decl(branch);
        let TyKind::Fresh(TyFresh(reference)) = params.params[0].0.kind else {
            panic!("expected fresh")
        };
        assert_eq!(reference.group, Some(expected_group));
    }
}

#[test]
fn three_axes_preserve_right_outer_order_and_shared_fresh_names() {
    reset_fresh_counter();
    let a = generated(2);
    let b = generated(2);
    let c = generated(1);
    let value = pack(vec![atom("Map")]).apply(a.clone()).apply(b.clone()).apply(c.clone());
    let (params, body) = split_decl(&value);
    assert_eq!(params.params.len(), 5);
    let refs =
        |v: &Ty| split_decl(v).0.params.iter().map(|(p, _)| p.as_ref().clone()).collect::<Vec<_>>();
    let (av, bv, cv) = (refs(&a), refs(&b), refs(&c));
    let rows = cv
        .into_iter()
        .map(|ci| {
            pack(
                bv.iter()
                    .map(|bi| {
                        pack(
                            av.iter()
                                .map(|ai| {
                                    pack(vec![
                                        atom("Map")
                                            .apply(ai.clone())
                                            .apply(bi.clone())
                                            .apply(ci.clone()),
                                    ])
                                })
                                .collect(),
                        )
                    })
                    .collect(),
            )
        })
        .collect();
    same(body, &pack(rows));
}

#[test]
fn pack_growth_uses_the_shared_mass_guard() {
    let left = pack(vec![atom("F"); MAX_EXPAND]);
    assert!(matches!(left.apply(atom("A")).kind, TyKind::Error(_)));
    assert!(matches!(pack(vec![]).apply(TyNum(MAX_EXPAND + 1).to_ty()).kind, TyKind::Error(_)));
}
