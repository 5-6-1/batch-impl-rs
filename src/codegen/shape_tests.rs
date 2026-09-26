//! Regression contracts for shape matching, independent of rendered impls.

use super::shape::{Mapping, match_shape};
use proc_macro2::TokenStream;
use quote::quote;
use std::collections::HashSet;

fn match_tokens(
    template: TokenStream, leaf: TokenStream, consts: &[&str],
) -> Result<Mapping, String> {
    let template = syn::parse2(template).unwrap();
    let leaf = syn::parse2(leaf).unwrap();
    let consts = consts.iter().map(|name| (*name).to_owned()).collect::<HashSet<_>>();
    match_shape(&template, &leaf, &consts).map(|(m, _)| m).map_err(|e| e.message())
}

#[test]
fn literal_and_binding_conflicts_are_order_independent() {
    for leaf in [quote!((u8, u16)), quote!((u16, u8))] {
        let error = match_tokens(quote!((u8, u8)), leaf, &[]).err().unwrap();
        assert!(error.contains("binding slot `u8` is bound to different subtrees"));
    }
}

#[test]
fn merged_templates_keep_literal_constraints() {
    for reverse in [false, true] {
        let literal = match_tokens(quote!(u8), quote!(u8), &[]).unwrap();
        let binding = match_tokens(quote!(u8), quote!(u16), &[]).unwrap();
        let (mut first, second) = if reverse { (binding, literal) } else { (literal, binding) };
        assert!(first.merge(second).is_err());
    }
}

#[test]
fn identity_positions_are_not_replacement_slots() {
    for template in [quote!(Wrap<T>), quote!([u8; N]), quote!(Wrap<N>)] {
        let map = match_tokens(template.clone(), template, &["N"]).unwrap();
        assert!(map.slots().is_empty());
    }
}

#[test]
fn const_argument_classification_uses_declarations() {
    let map = match_tokens(quote!(Wrap<N>), quote!(Wrap<2>), &["N"]).unwrap();
    assert_eq!(
        map.slots().iter().map(|(n, v)| (n.as_str(), v.to_string())).collect::<Vec<_>>(),
        vec![("N", "2".into())]
    );
    assert!(match_tokens(quote!(Wrap<N>), quote!(Wrap<2>), &[]).is_err());
    assert!(match_tokens(quote!(Wrap<N>), quote!(Wrap<u8>), &["N"]).is_err());
    assert!(match_tokens(quote!(Wrap<T>), quote!(Wrap<N>), &["N"]).is_err());
    assert!(match_tokens(quote!(Wrap<N>), quote!(Wrap<M>), &["N", "M"]).is_ok());
    assert!(match_tokens(quote!(Wrap<N>), quote!(Wrap<{ 1 + 2 }>), &["N"]).is_ok());
}

#[test]
fn function_arguments_and_returns_bind_structurally() {
    let map = match_tokens(quote!(fn(arg: A) -> B), quote!(fn(value: u8) -> u16), &[]).unwrap();
    assert_eq!(map.slots().len(), 2);
    assert!(map.slots().iter().all(|(n, _)| n == "A" || n == "B"));
    let unit = match_tokens(quote!(fn(A) -> R), quote!(fn(u8)), &[]).unwrap();
    assert!(unit.slots().iter().any(|(n, v)| n == "R" && v.to_string() == "()"));
    assert!(match_tokens(quote!(fn(u8)), quote!(fn(u8) -> ()), &[]).is_ok());
    let map = match_tokens(quote!(A), quote!(u16), &[]).unwrap();
    let named = super::shape::apply_type_mapping(quote!(fn(#[cfg(any())] A: A, ...)), &map);
    assert_eq!(named.to_string(), quote!(fn(#[cfg(any())] A: u16, ...)).to_string());
    // Type macros can contain non-Rust fn-like tokens. An unsuccessful fn-type
    // parse falls back to the lexical rewrite without inventing a diagnostic.
    let mac = super::shape::apply_type_mapping(quote!(TypeMacro!(fn(A => value))), &map);
    assert_eq!(mac.to_string(), quote!(TypeMacro!(fn(u16 => value))).to_string());
}

#[test]
fn function_qualifiers_and_lifetimes_cannot_silently_change() {
    for (template, leaf) in [
        (quote!(fn(A)), quote!(unsafe fn(u8))),
        (quote!(extern "C" fn(A)), quote!(extern "system" fn(u8))),
        (quote!(extern "C" fn(A, ...)), quote!(extern "C" fn(u8))),
        (quote!(fn(A)), quote!(fn(u8, u16))),
        (quote!(for<'a> fn(&'a A)), quote!(for<'b> fn(&'b u8))),
        (quote!(for<'a> fn(&'a A)), quote!(for<'a> fn(&'static u8))),
        (quote!(fn(Cow<'_, A>)), quote!(fn(Cow<'static, u8>))),
    ] {
        assert!(match_tokens(template.clone(), leaf.clone(), &[]).is_err(), "{template} vs {leaf}");
    }
    assert!(
        match_tokens(
            quote!(for<'a> unsafe extern "C" fn(&'a A, ...) -> B),
            quote!(for<'a> unsafe extern "C" fn(&'a u8, ...) -> u16),
            &[]
        )
        .is_ok()
    );
}
