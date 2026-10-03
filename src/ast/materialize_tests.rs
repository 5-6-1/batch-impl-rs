use super::*;
use quote::{ToTokens, quote};

fn atom(name: &str) -> Ty {
    TyPrimitive(name.parse().unwrap()).to_ty()
}
fn pack(items: Vec<Ty>) -> Ty {
    TyPack(items).to_ty()
}
fn tuple(items: Vec<Ty>) -> Ty {
    TyTuple(items).to_ty()
}
fn choices(items: Vec<Ty>) -> Ty {
    TyArray(items).to_ty()
}
fn generic(name: &str, args: Vec<Ty>) -> Ty {
    TyGeneric(
        atom(name).into(),
        TyTypeParam {
            params: args.into_iter().map(|arg| (arg.into(), None)).collect(),
            bindings: vec![],
        },
    )
    .to_ty()
}
fn output(value: Ty) -> Vec<String> {
    materialize_targets(value)
        .unwrap()
        .into_iter()
        .map(|ty| ty.to_token_stream().to_string())
        .collect()
}
fn expect(value: Ty, expected: &[proc_macro2::TokenStream]) {
    assert_eq!(output(value), expected.iter().map(ToString::to_string).collect::<Vec<_>>());
}
fn carrier(name: &str, body: Ty) -> Ty {
    TyWithType(TyTypeParam::single(&atom(name)), body.into()).to_ty()
}

#[test]
fn nested_packs_splice_without_opening_ordinary_tuples() {
    expect(
        tuple(vec![pack(vec![atom("A"), pack(vec![atom("B")])]), tuple(vec![atom("C")])]),
        &[quote!((A, B, (C,),))],
    );
    expect(pack(vec![atom("A"), tuple(vec![atom("B"), atom("C")])]), &[quote!(A), quote!((B, C,))]);
}

#[test]
fn candidate_packs_preserve_variable_length_branches() {
    let rows = choices(vec![pack(vec![atom("A")]), pack(vec![atom("B"), atom("C")])]);
    expect(tuple(vec![rows]), &[quote!((A,)), quote!((B, C,))]);
    expect(
        tuple(vec![choices(vec![atom("A"), atom("B")]), choices(vec![atom("C"), atom("D")])]),
        &[quote!((A, C,)), quote!((A, D,)), quote!((B, C,)), quote!((B, D,))],
    );
}

#[test]
fn direct_generic_hosts_collect_slots_without_reapplying() {
    expect(
        generic("F", vec![pack(vec![atom("A"), atom("B")]), pack(vec![atom("C"), atom("D")])]),
        &[quote!(F<A,B,C,D>)],
    );
    expect(generic("F", vec![tuple(vec![atom("A"), atom("B")])]), &[quote!(F<(A, B,)>)]);
}

#[test]
fn single_slot_hosts_reject_zero_or_multiple_types() {
    for items in [vec![], vec![atom("A"), atom("B")]] {
        let inner = pack(items);
        for host in [
            TyWithPrefix(TyPrefix::Ref, inner.clone().into()).to_ty(),
            TyWithPrefix(TyPrefix::PtrConst, inner.clone().into()).to_ty(),
            TyPrimitiveArray(inner.clone().into(), None).to_ty(),
            TyPrimitiveArray(inner.clone().into(), Some(quote!(3))).to_ty(),
            TyFn(Some(vec![]), inner.clone().into(), false, FnKind::Bare).to_ty(),
            TyPrefixed(quote!(&'a mut), inner.into()).to_ty(),
        ] {
            assert!(
                materialize_targets(host)
                    .unwrap_err()
                    .to_string()
                    .contains("requires exactly one type")
            );
        }
    }
}

#[test]
fn single_slots_accept_each_candidate_and_callable_parameters_are_variadic() {
    let inner = choices(vec![pack(vec![atom("A")]), pack(vec![atom("B")])]);
    expect(TyPrefixed(quote!(&'a), inner.into()).to_ty(), &[quote!(&'a A), quote!(&'a B)]);
    expect(
        TyFn(
            Some(vec![pack(vec![atom("A"), atom("B")])]),
            pack(vec![atom("R")]).into(),
            false,
            FnKind::Bare,
        )
        .to_ty(),
        &[quote!(fn(A, B) -> R)],
    );
}

#[test]
fn empty_hosts_keep_declarations_and_bare_empty_packs_emit_no_targets() {
    expect(tuple(vec![carrier("T", pack(vec![]))]), &[quote!(<T>())]);
    assert!(output(carrier("T", pack(vec![]))).is_empty());
    expect(pack(vec![carrier("T", atom("A")), atom("B")]), &[quote!(<T>A), quote!(<T>B)]);
}

#[test]
fn tuple_copies_keep_fresh_identity_without_allocating_during_materialization() {
    reset_fresh_counter();
    let source = tuple(vec![]).apply(TyNum(1).to_ty());
    let result = materialize_targets(tuple(vec![source.clone(), source])).unwrap();
    assert_eq!(take_group(), 1);
    let TyKind::WithType(params) = &result[0].kind else { panic!("carrier missing") };
    assert_eq!(params.0.params.len(), 2);
    assert_eq!(
        params.0.params[0].0.to_token_stream().to_string(),
        params.0.params[1].0.to_token_stream().to_string()
    );
}

#[test]
fn declaration_names_and_bounds_have_distinct_slot_rules() {
    let params = TyTypeParam {
        params: vec![(pack(vec![atom("T"), atom("U")]).into(), Some(atom("Clone").into()))],
        bindings: vec![],
    };
    expect(
        TyWithType(params, tuple(vec![atom("T"), atom("U")]).into()).to_ty(),
        &[quote!(<T:Clone,U:Clone>(T,U,))],
    );
    assert!(
        materialize_targets(carrier("Vec<u8>", atom("u8")))
            .unwrap_err()
            .to_string()
            .contains("parameter name")
    );
    let params = TyTypeParam {
        params: vec![(atom("T").into(), Some(pack(vec![atom("Clone"), atom("Copy")]).into()))],
        bindings: vec![],
    };
    assert!(
        materialize_targets(TyWithType(params, atom("T").into()).to_ty())
            .unwrap_err()
            .to_string()
            .contains("requires exactly one type")
    );
}

#[test]
fn bound_choices_hoist_only_the_selected_branchs_fresh_declarations() {
    reset_fresh_counter();
    let generated = tuple(vec![]).apply(TyNum(1).to_ty());
    let bounds = choices(vec![atom("Clone"), generic("Tr", vec![generated])]);
    let params =
        TyTypeParam { params: vec![(atom("T").into(), Some(bounds.into()))], bindings: vec![] };
    let result = materialize_targets(TyWithType(params, atom("T").into()).to_ty()).unwrap();
    assert_eq!(result.len(), 2);
    let declarations = result
        .iter()
        .map(|ty| match &ty.kind {
            TyKind::WithType(w) => w.0.params.len(),
            _ => 0,
        })
        .collect::<Vec<_>>();
    assert_eq!(declarations, vec![1, 2]);
    assert_eq!(take_group(), 1);
}

#[test]
fn declaration_generators_keep_the_existing_targeted_diagnostic() {
    reset_fresh_counter();
    let generated = pack(vec![]).apply(TyNum(2).to_ty());
    let params = TyTypeParam::single(&generated);
    let error = materialize_targets(TyWithType(params, atom("u8").into()).to_ty()).unwrap_err();
    assert!(error.to_string().contains("fresh generator cannot be declared here"));
}

#[test]
fn trait_arguments_and_associated_bindings_distribute_independently() {
    let args = TyTypeParam {
        params: vec![(pack(vec![atom("A"), atom("B")]).into(), None)],
        bindings: vec![(atom("Item").into(), choices(vec![atom("C"), atom("D")]).into())],
    };
    let ty = TyWithTrait(TyTrait(quote!(Tr), args), atom("T").into()).to_ty();
    expect(ty, &[quote!(Tr < A, B, Item = C > T), quote!(Tr < A, B, Item = D > T)]);
    let args =
        TyTypeParam { params: vec![], bindings: vec![(atom("Item").into(), pack(vec![]).into())] };
    assert!(
        materialize_targets(TyTrait(quote!(Tr), args).to_ty())
            .unwrap_err()
            .to_string()
            .contains("requires exactly one type")
    );
}

#[test]
fn ordinary_wrappers_and_qualified_heads_preserve_their_structure() {
    let tr = generic("Tr", vec![pack(vec![atom("A"), atom("B")])]);
    expect(
        TyWithDyn(tr.clone().into(), TyBoundList(vec![atom("Send")])).to_ty(),
        &[quote!(dyn Tr<A, B> + Send)],
    );
    expect(
        TyWithFor(vec![TyLifetime(quote!('a)).to_ty()], tr.clone().into()).to_ty(),
        &[quote!(for<'a> Tr<A, B>)],
    );
    expect(
        TyQualified(QualifiedHead::Type(tr.into()), vec![quote!(Assoc)]).to_ty(),
        &[quote!(Tr < A, B > ::Assoc)],
    );
    expect(
        TyWithCode(
            pack(vec![atom("A"), atom("B")]).into(),
            TyCodeBlock(quote!(
                fn f() {}
            )),
        )
        .to_ty(),
        &[quote!(A {fn f(){}}), quote!(B {fn f(){}})],
    );
}

#[test]
fn cartesian_materialization_checks_limits_before_building_outputs() {
    let axis = choices(vec![atom("A"); 33]);
    assert!(
        materialize_targets(tuple(vec![axis.clone(), axis]))
            .unwrap_err()
            .to_string()
            .contains("limit 1024")
    );
    let deep =
        (0..crate::util::MAX_NEST_DEPTH + 2).fold(atom("T"), |t, _| TyGroup(t.into()).to_ty());
    assert!(materialize_targets(deep).unwrap_err().to_string().contains("nesting depth"));
}

#[test]
fn the_existing_target_count_boundary_and_empty_choices_are_preserved() {
    assert_eq!(
        materialize_targets(choices(vec![atom("T"); MAX_EXPAND])).unwrap().len(),
        MAX_EXPAND
    );
    assert!(materialize_targets(choices(vec![atom("T"); MAX_EXPAND + 1])).is_err());
    let complex = generic("Vec", vec![tuple(vec![atom("u8"), atom("u16")])]);
    assert_eq!(materialize_targets(choices(vec![complex; MAX_EXPAND])).unwrap().len(), MAX_EXPAND);
    assert!(
        materialize_targets(tuple(vec![carrier("T", choices(vec![])), atom("U")]))
            .unwrap()
            .is_empty()
    );
    expect(tuple(vec![pack(vec![])]), &[quote!(())]);
    let zero_axis = choices(vec![pack(vec![]); 33]);
    assert!(
        materialize_targets(tuple(vec![zero_axis.clone(), zero_axis]))
            .unwrap_err()
            .to_string()
            .contains("limit 1024")
    );
}

#[test]
fn metadata_and_late_fresh_references_survive_collection() {
    let fresh = TyFresh(FreshRef { group: None, start: 0, end: FreshEnd::Open }).to_ty();
    let target = TyWithType(
        TyTypeParam::single(&fresh),
        TyWithImpl(
            TyWithWhere(
                TyWithAttr(TyAttr(quote!(allow(dead_code))), tuple(vec![fresh.clone()]).into())
                    .to_ty()
                    .into(),
                TyWhere(quote!(T:Clone)),
            )
            .to_ty()
            .into(),
            TyImplTemplate(quote!(T)),
        )
        .to_ty()
        .into(),
    )
    .to_ty();
    let targets = materialize_targets(target).unwrap();
    assert_eq!(targets.len(), 1);
    let TyKind::WithType(w) = &targets[0].kind else { panic!("declaration missing") };
    assert!(matches!(w.0.params[0].0.kind, TyKind::Fresh(_)));
    let TyKind::WithImpl(w) = &w.1.kind else { panic!("template missing") };
    assert_eq!(w.1.0.to_string(), "T");
    assert!(w.0.as_ref().unwrap().to_token_stream().to_string().contains("where T : Clone"));
}

#[test]
fn cumulative_materialization_work_has_a_separate_budget() {
    let complex = (0..20).fold(atom("u8"), |inner, _| generic("Vec", vec![inner]));
    assert!(count_leaves(&complex) < MAX_EXPAND);
    let error = materialize_targets(choices(vec![complex; MAX_EXPAND])).unwrap_err();
    assert!(error.to_string().contains("needs too much materialization work"), "{error}");
}
