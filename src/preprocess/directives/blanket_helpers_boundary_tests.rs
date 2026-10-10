use super::blanket_helpers::*;
use proc_macro2::{TokenStream, TokenTree};
use quote::quote;

fn tokens(source: &str) -> Vec<TokenTree> {
    source.parse::<TokenStream>().unwrap().into_iter().collect()
}

#[test]
fn target_marker_is_only_detected_in_type_structure() {
    assert!(has_at0(&tokens("Box<(@0,)>")));
    for source in [
        "Wrap<{ let _ = stringify!(@0); 3 }>",
        "type_macro!(@0)",
        "type_macro![@0]",
        "type_macro!{@0}",
        "fn(#[attribute(@0)] u8)",
        "Wrap<@{@0}>",
    ] {
        assert!(!has_at0(&tokens(source)), "{source}");
    }
}

#[test]
fn wrapper_predicates_preserve_rust_blocks_macros_and_attributes() {
    for source in [
        "[u8; { match 2 { n @ ..=3 => n, _ => 0 } }]: Sized",
        "type_macro!(@trait): Sized",
        "type_macro![@trait]: Sized",
        "type_macro!{@trait}: Sized",
        "fn(#[attribute(@trait)] u8): Sized",
    ] {
        let input = tokens(source);
        let expected = input.iter().cloned().collect::<TokenStream>().to_string();
        let output = resolve_target_predicates(&input, &quote!(::external::Trait<u8>))
            .unwrap_or_else(|error| panic!("{source}: {error}"));
        assert_eq!(output.into_iter().collect::<TokenStream>().to_string(), expected);
    }
}

#[test]
fn wrapper_predicates_keep_carriers_and_resolve_visible_trait_markers() {
    let input = tokens("@{0}: @trait, @0: @trait");
    let output = resolve_target_predicates(&input, &quote!(::external::Trait<u8>)).unwrap();
    let expected: TokenStream =
        "@{0}: ::external::Trait<u8> , @0: ::external::Trait<u8>".parse().unwrap();
    assert_eq!(output.into_iter().collect::<TokenStream>().to_string(), expected.to_string());
    assert!(resolve_target_predicates(&tokens("@unknown: Sized"), &quote!(Trait)).is_err());
}
