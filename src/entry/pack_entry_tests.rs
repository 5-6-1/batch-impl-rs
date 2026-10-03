//! Integration regressions at the public attribute entry, after parsing,
//! collection, metadata extraction and final fresh-name assignment.

use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

use super::{expand_attr_macro, expand_impl_entry};

fn trait_output(spec: TokenStream) -> TokenStream {
    expand_attr_macro(
        spec,
        syn::parse_quote!(
            trait Marker {}
        ),
        false,
    )
    .unwrap()
}

fn impls(tokens: TokenStream) -> Vec<syn::ItemImpl> {
    assert!(!tokens.to_string().contains("compile_error"), "{tokens}");
    syn::parse2::<syn::File>(tokens)
        .unwrap()
        .items
        .into_iter()
        .map(|item| {
            let syn::Item::Impl(item) = item else { panic!("expected impl") };
            item
        })
        .collect()
}

#[test]
fn lifted_declarations_preserve_attributes_and_where_scope_per_range_branch() {
    let out = trait_output(quote!(
        #[allow(dead_code)] <'a> &'a (*Vec *[].1..=2,) where { @0..: Clone }
    ));
    let items = impls(out);
    assert_eq!(items.len(), 2);
    for (item, arity) in items.iter().zip(1..) {
        assert_eq!(item.attrs.len(), 1);
        assert_eq!(item.generics.params.len(), arity + 1);
        assert_eq!(item.generics.where_clause.as_ref().unwrap().predicates.len(), arity);
        let target = item.self_ty.to_token_stream().to_string();
        assert!(target.starts_with("& 'a"), "{target}");
        assert_eq!(target.matches("Vec").count(), arity);
    }
}

#[test]
fn copying_generated_slots_reuses_one_generic_after_the_full_pipeline() {
    let items = impls(trait_output(quote!((*[].1,).2)));
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].generics.params.len(), 1);
    let syn::Type::Tuple(tuple) = items[0].self_ty.as_ref() else { panic!("expected tuple") };
    assert_eq!(tuple.elems.len(), 2);
    assert_eq!(
        tuple.elems[0].to_token_stream().to_string(),
        tuple.elems[1].to_token_stream().to_string()
    );
}

#[test]
fn named_abi_function_parameters_keep_their_single_slot_boundary() {
    let items = impls(trait_output(quote!(extern "C" fn(value: *[u8,]) -> *[u16,])));
    assert_eq!(items.len(), 1);
    let syn::Type::FnPtr(function) = items[0].self_ty.as_ref() else { panic!("expected fn") };
    assert_eq!(function.abi.as_ref().unwrap().name.as_ref().unwrap().value(), "C");
    assert_eq!(function.inputs[0].name.as_ref().unwrap().0.to_string(), "value");
    let out = trait_output(quote!(extern "C" fn(value: *[u8, u16])));
    assert!(out.to_string().contains("requires exactly one type"), "{out}");
}

#[test]
fn public_empty_collections_and_an_explicit_empty_row_differ() {
    // A spec with no targets at all is diagnosed, whichever way it is spelled: an
    // empty collection is not a silent no-op.
    for spec in [quote!([*[],]), quote!(*F * [])] {
        let out = trait_output(spec);
        assert!(out.to_string().contains("expands to zero impls"), "{out}");
    }
    let tuple = impls(trait_output(quote!((*[],))));
    assert_eq!(tuple.len(), 1);
    assert_eq!(tuple[0].self_ty.to_token_stream().to_string(), "()");
    let row = impls(trait_output(quote!(*F * [*[],])));
    assert_eq!(row.len(), 1);
    assert_eq!(row[0].self_ty.to_token_stream().to_string(), "F");
}

#[test]
fn the_spec_limit_counts_pack_targets_from_every_top_level_candidate() {
    let members = vec![quote!(u8); 64];
    let one_pack = quote!(*[#(#members,)*]);
    let packs = vec![one_pack.clone(); 16];
    assert_eq!(impls(trait_output(quote!([#(#packs),*]))).len(), 1024);
    let packs = vec![one_pack; 17];
    let out = trait_output(quote!([#(#packs),*]));
    assert!(out.to_string().contains("the spec expands to 1088 impls"), "{out}");
    assert!(!out.to_string().contains("impl Marker"), "partial impls escaped: {out}");
}

#[test]
fn impl_leaf_predicates_cannot_be_shadowed_by_lifted_fresh_parameters() {
    let out = expand_impl_entry(
        quote!(@Self: ((*Vec *[].1,) where { u8: Bound<P0> })),
        syn::parse_quote!(impl Marker for Prototype {}),
    )
    .unwrap();
    let items = impls(out);
    assert_eq!(items.len(), 1);
    let syn::GenericParam::Type(parameter) = &items[0].generics.params[0] else {
        panic!("type parameter")
    };
    assert_eq!(parameter.ident.to_string(), "P0A");
    assert!(items[0].generics.where_clause.to_token_stream().to_string().contains("Bound < P0 >"));
}

#[test]
fn impl_generator_bounds_are_retained_and_participate_in_fresh_naming() {
    for spec in [quote!(@Self: (*Vec *[<Bound<P0>>,].2,)), quote!((*Vec *[<Bound<P0>>,].2,))] {
        let out = expand_impl_entry(spec, syn::parse_quote!(impl Marker for Prototype {})).unwrap();
        let items = impls(out);
        assert_eq!(items.len(), 1);
        let item = &items[0];
        assert_eq!(item.generics.params.len(), 2);
        let predicates = &item.generics.where_clause.as_ref().unwrap().predicates;
        assert_eq!(predicates.len(), 2);
        assert_eq!(predicates[0].to_token_stream().to_string(), "P0A : Bound < P0 >");
        assert_eq!(predicates[1].to_token_stream().to_string(), "P1 : Bound < P0 >");
        assert_eq!(item.self_ty.to_token_stream().to_string(), "(Vec < P0A > , Vec < P1 > ,)");
    }
}

#[test]
fn impl_generator_bounds_resolve_references_against_the_same_fresh_context() {
    for spec in [quote!(@Self: (*Vec *[<Bound<@0>>,].1,)), quote!((*Vec *[<Bound<@0>>,].1,))] {
        let out = expand_impl_entry(spec, syn::parse_quote!(impl Marker for Prototype {})).unwrap();
        let items = impls(out);
        let predicates = &items[0].generics.where_clause.as_ref().unwrap().predicates;
        assert_eq!(predicates.len(), 1);
        assert_eq!(predicates[0].to_token_stream().to_string(), "P0 : Bound < P0 >");
    }
}

#[test]
fn reference_and_bound_modifiers_do_not_restart_pack_pairing() {
    let cases: [(TokenStream, syn::Type); 6] = [
        (quote!((*[&*Pair,] * [*[u8, u16],],)), syn::parse_quote!((&Pair<u8, u16>,))),
        (
            quote!((*[&'static *Pair,] *[*[u8, u16],],)),
            syn::parse_quote!((&'static Pair<u8, u16>,)),
        ),
        (quote!((*[&mut *Pair,] * [*[u8, u16],],)), syn::parse_quote!((&mut Pair<u8, u16>,))),
        (quote!((*[*const *Pair,] *[*[u8, u16],],)), syn::parse_quote!((*const Pair<u8, u16>,))),
        (
            quote!((*[dyn *Pair + Send,] *[*[u8, u16],],)),
            syn::parse_quote!((dyn Pair<u8, u16> + Send,)),
        ),
        (
            quote!((*[dyn for<'a> *Pair + Send,] *[*[u8, u16],],)),
            syn::parse_quote!((dyn for<'a> Pair<u8, u16> + Send,)),
        ),
    ];
    for (spec, expected) in cases {
        let items = impls(trait_output(spec.clone()));
        assert_eq!(items.len(), 1, "{spec}");
        assert_eq!(
            items[0].self_ty.to_token_stream().to_string(),
            expected.to_token_stream().to_string(),
            "{spec}"
        );
    }
}

#[test]
fn malformed_function_returns_keep_the_original_diagnostic_through_the_entry() {
    for spec in [
        quote!(extern "C" fn(u8) -> u8 #(x)),
        quote!(unsafe extern "C" fn(u8) -> u8 #(x)),
        quote!(fn(u8) -> u8 #(x)),
        quote!(dyn Fn(u8) -> u8 #(x)),
    ] {
        let output = trait_output(spec.clone()).to_string();
        assert!(output.contains("unexpected `#` in a type position"), "{spec}: {output}");
        assert!(!output.contains("already has a return type"), "{spec}: {output}");
    }
}
