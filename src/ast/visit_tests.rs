//! Traversal regressions for packs, declaration carriers and nested hosts.

use quote::{ToTokens, quote};

use crate::apply::err_ty;
use crate::ast::*;

fn primitive(name: &str) -> Ty {
    TyPrimitive(name.parse().unwrap()).to_ty()
}

fn numbers(ty: Ty, seen: &mut Vec<usize>) -> Ty {
    let ty = ty.map_children(&mut |child| numbers(child, seen));
    if let TyKind::Num(n) = &ty.kind {
        seen.push(n.0);
    }
    ty
}

#[test]
fn pack_walk_preserves_nested_choices_and_visits_their_members() {
    let nested =
        TyPack(vec![TyNum(1).to_ty(), TyArray(vec![TyNum(2).to_ty(), TyNum(3).to_ty()]).to_ty()])
            .to_ty();
    let mut seen = vec![];
    let original = nested.to_token_stream().to_string();
    let rebuilt = numbers(nested, &mut seen);
    assert_eq!(seen, [1, 2, 3]);
    assert_eq!(count_leaves(&rebuilt), 5);
    assert_eq!(rebuilt.to_token_stream().to_string(), original);
    assert!(matches!(rebuilt.kind, TyKind::Pack(_)));
}

#[test]
fn pack_walk_preserves_rows_including_an_empty_row() {
    let nested = TyPack(vec![
        TyPack(vec![TyNum(1).to_ty(), TyNum(2).to_ty()]).to_ty(),
        TyPack(vec![]).to_ty(),
    ])
    .to_ty();
    let mut seen = vec![];
    let rebuilt = numbers(nested, &mut seen);
    assert_eq!(seen, [1, 2]);
    assert_eq!(count_leaves(&rebuilt), 5);
    let TyKind::Pack(outer) = rebuilt.kind else { panic!("lost the outer pack") };
    assert_eq!(outer.0.len(), 2);
    assert!(matches!(&outer.0[1].kind, TyKind::Pack(p) if p.0.is_empty()));
    assert_eq!(count_leaves(&TyPack(vec![]).to_ty()), 1);
}

#[test]
fn standalone_parameter_and_trait_nodes_visit_bounds_and_bindings() {
    let tp = TyTypeParam {
        params: vec![(TyNum(1).into(), Some(TyNum(2).into()))],
        bindings: vec![(TyNum(3).into(), TyNum(4).into())],
    };
    for ty in [tp.clone().to_ty(), TyTrait(quote!(Trait), tp).to_ty()] {
        let mut seen = vec![];
        let rebuilt = numbers(ty, &mut seen);
        assert_eq!(seen, [1, 2, 3, 4]);
        assert_eq!(count_leaves(&rebuilt), 5);
    }
}

#[test]
fn list_growth_guard_counts_mass_hidden_inside_a_pack() {
    let splat = TyPack(vec![primitive("T"); MAX_EXPAND]).to_ty();
    assert_eq!(count_leaves(&splat), MAX_EXPAND + 1);
    let result = primitive("F").apply(TyArray(vec![splat]).to_ty());
    assert!(matches!(result.kind, TyKind::Error(_)));
    assert!(result.to_token_stream().to_string().contains("expansion mass"));
}

#[test]
fn errors_nested_in_splat_arguments_stop_the_public_pipeline() {
    let output = crate::entry::expand_attr_macro(
        quote!(Wrapper<*(4 T,)>),
        syn::parse_quote!(
            trait Example {}
        ),
        true,
    )
    .unwrap();
    let file = syn::parse_file(&output.to_string()).unwrap();
    assert_eq!(file.items.len(), 1);
    assert!(matches!(file.items[0], syn::Item::Macro(_)));
    assert!(output.to_string().contains("number `4` cannot be a left operand"));
}

#[test]
fn traversal_keeps_nested_errors_and_shared_fresh_identity() {
    reset_fresh_counter();
    let generated = TyTuple(vec![]).to_ty().apply(TyNum(1).to_ty());
    let target = TyPack(vec![generated.clone(), generated, err_ty("sentinel")]).to_ty();
    let mut decls = vec![];
    let rebuilt = crate::codegen::hoist_type_params(target, &mut decls);
    assert_eq!(decls.len(), 1, "copies share one structured fresh identity");
    assert!(rebuilt.to_token_stream().to_string().contains("sentinel"));
    assert_eq!(count_leaves(&rebuilt), 6);
}
