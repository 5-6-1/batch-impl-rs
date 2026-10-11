//! Row identity and scope contracts for composite variadic shape patterns.

use super::shape::{Mapping, VarSeg, match_shape, merge_segments};
use proc_macro2::TokenStream;
use quote::quote;

fn matched(template: TokenStream, leaf: TokenStream) -> Result<(Mapping, Vec<VarSeg>), String> {
    matched_with_consts(template, leaf, &[])
}

fn matched_with_consts(
    template: TokenStream, leaf: TokenStream, consts: &[&str],
) -> Result<(Mapping, Vec<VarSeg>), String> {
    let template = syn::parse2(template).unwrap();
    let leaf = syn::parse2(leaf).unwrap();
    let consts = consts.iter().map(|name| (*name).to_owned()).collect();
    match_shape(&template, &leaf, &consts).map_err(|error| error.message())
}

fn capture(row: &Mapping, name: &str) -> String {
    row.capture(name).unwrap().to_string()
}

#[test]
fn dense_constructor_matrix_keeps_all_eighteen_ordered_rows() {
    let mut elements = Vec::new();
    let mut expected = Vec::new();
    for ty in ["u8", "u16", "u32", "u64", "u128", "usize"] {
        for constructor in ["Vec", "Rc", "Box"] {
            elements.push(syn::parse_str::<syn::Type>(&format!("{constructor}<{ty}>")).unwrap());
            expected.push((constructor.to_owned(), ty.to_owned()));
        }
    }
    let (fixed, segments) = matched(quote!(([A<B>; ()],)), quote!((#(#elements,)*))).unwrap();
    assert!(fixed.captures().next().is_none());
    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].names, ["A", "B"]);
    assert_eq!(segments[0].len(), 18);
    let actual = segments[0]
        .rows
        .iter()
        .map(|row| (capture(row, "A"), capture(row, "B")))
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
}

#[test]
fn sparse_pairs_are_neither_crossed_nor_deduplicated() {
    let (_, segments) =
        matched(quote!(([A<B>; ()],)), quote!((Vec<u8>, Rc<u16>, Vec<u8>))).unwrap();
    let rows = &segments[0].rows;
    assert_eq!(rows.len(), 3);
    assert_eq!(capture(&rows[0], "A"), "Vec");
    assert_eq!(capture(&rows[0], "B"), "u8");
    assert_eq!(capture(&rows[1], "A"), "Rc");
    assert_eq!(capture(&rows[1], "B"), "u16");
    assert_eq!(capture(&rows[2], "A"), "Vec");
    assert_eq!(capture(&rows[2], "B"), "u8");
}

#[test]
fn identity_captures_exist_in_each_row_without_becoming_fixed_slots() {
    let (_, segments) = matched(quote!(([A<B>; ()],)), quote!((A<B>, Vec<B>))).unwrap();
    let rows = &segments[0].rows;
    assert!(rows[0].slots().is_empty());
    assert_eq!(capture(&rows[0], "A"), "A");
    assert_eq!(capture(&rows[0], "B"), "B");
    assert_eq!(capture(&rows[1], "A"), "Vec");
    assert_eq!(capture(&rows[1], "B"), "B");
}

#[test]
fn empty_segments_keep_the_pattern_capture_schema() {
    let (_, segments) = matched(quote!(([A<B>; ()],)), quote!(())).unwrap();
    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].names, ["A", "B"]);
    assert_eq!(segments[0].start, 0);
    assert_eq!(segments[0].len(), 0);
    let (_, segments) = matched(quote!((u8, [_; ()])), quote!((u8,))).unwrap();
    assert!(segments[0].names.is_empty());
    assert_eq!(segments[0].start, 1);
    assert_eq!(segments[0].len(), 0);
}

#[test]
fn repeated_names_constrain_one_row_but_not_other_rows() {
    let template = quote!(([Pair<T, T>; ()],));
    let (_, segments) = matched(template.clone(), quote!((Pair<u8, u8>, Pair<u16, u16>))).unwrap();
    assert_eq!(capture(&segments[0].rows[0], "T"), "u8");
    assert_eq!(capture(&segments[0].rows[1], "T"), "u16");
    for leaf in [quote!((Pair<u8, u16>,)), quote!((Pair<T, u8>,))] {
        let error = matched(template.clone(), leaf).unwrap_err();
        assert!(error.contains("binding slot `T` is bound to different subtrees"));
    }
}

#[test]
fn fixed_slots_and_same_named_row_captures_are_separate() {
    let (fixed, segments) =
        matched(quote!((T, [Vec<T>; ()])), quote!((u8, Vec<T>, Box<u16>))).unwrap();
    assert_eq!(capture(&fixed, "T"), "u8");
    assert_eq!(segments[0].start, 1);
    assert_eq!(capture(&segments[0].rows[0], "T"), "T");
    assert_eq!(capture(&segments[0].rows[1], "T"), "u16");
    assert_eq!(capture(&segments[0].rows[1], "Vec"), "Box");
}

#[test]
fn multi_capture_patterns_count_as_one_segment_when_splitting() {
    let (_, segments) = matched(
        quote!(([Pair<K, V>; ()], [Box<T>; ()])),
        quote!((Pair<u8, u16>, Pair<u32, u64>, Box<bool>, Box<char>)),
    )
    .unwrap();
    assert_eq!(segments.len(), 2);
    assert_eq!((segments[0].start, segments[0].len()), (0, 2));
    assert_eq!((segments[1].start, segments[1].len()), (2, 2));
    assert_eq!(capture(&segments[0].rows[1], "K"), "u32");
    assert_eq!(capture(&segments[0].rows[1], "V"), "u64");
    assert_eq!(capture(&segments[1].rows[1], "T"), "char");
}

#[test]
fn sibling_composite_segments_can_share_constructor_names() {
    let (_, segments) =
        matched(quote!(([Vec<T>; ()], [Vec<U>; ()])), quote!((Vec<u8>, Vec<u16>))).unwrap();
    assert_eq!(segments.len(), 2);
    assert_eq!(segments[0].names, ["Vec", "T"]);
    assert_eq!(segments[1].names, ["Vec", "U"]);
    let error = matched(quote!(([T; ()], [T; ()])), quote!((u8, u16))).unwrap_err();
    assert!(error.contains("duplicate variadic segment prefix `T`"));
}

#[test]
fn nested_segments_are_rejected_even_when_the_outer_segment_is_empty() {
    for pattern in [
        quote!(Vec<[T; ()]>),
        quote!(([T; ()],)),
        quote!(&[T; ()]),
        quote!(fn([T; ()])),
        quote!(dyn Bound<[T; ()]>),
    ] {
        let error = matched(quote!(([#pattern; ()],)), quote!(())).unwrap_err();
        assert!(error.contains("nested variadic segments inside a repeated pattern"), "{error}");
    }
    let (_, segments) =
        matched(quote!((([A; ()],), ([B; ()],))), quote!(((u8, u16), (bool,)))).unwrap();
    assert_eq!(segments.len(), 2);
    assert_eq!((segments[0].len(), segments[1].len()), (2, 1));
}

#[test]
fn repeated_patterns_reuse_existing_structural_and_const_matching() {
    for (pattern, value, names) in [
        (quote!((T, U)), quote!((u8, u16)), vec![("T", "u8"), ("U", "u16")]),
        (quote!(&mut T), quote!(&mut u8), vec![("T", "u8")]),
        (quote!(*const T), quote!(*const u8), vec![("T", "u8")]),
        (quote!([T; N]), quote!([u8; 3]), vec![("T", "u8"), ("N", "3")]),
        (quote!(fn(label: T) -> U), quote!(fn(u8) -> u16), vec![("T", "u8"), ("U", "u16")]),
        (quote!(Wrap<N>), quote!(Wrap<3>), vec![("Wrap", "Wrap"), ("N", "3")]),
    ] {
        let (_, segments) =
            matched_with_consts(quote!(([#pattern; ()],)), quote!((#value,)), &["N"]).unwrap();
        assert_eq!(segments[0].names.len(), names.len());
        for (name, value) in names {
            assert_eq!(capture(&segments[0].rows[0], name), value);
        }
    }
}

#[test]
fn tuple_valued_generic_slots_register_the_segment_driver() {
    let (_, segments) =
        matched(quote!(Wrap<[Vec<T>; ()]>), quote!(Wrap<(Vec<u8>, Box<u16>)>)).unwrap();
    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].len(), 2);
    assert_eq!(capture(&segments[0].rows[1], "Vec"), "Box");
    assert_eq!(capture(&segments[0].rows[1], "T"), "u16");
}

#[test]
fn marker_like_tokens_in_const_blocks_macros_and_attributes_remain_opaque() {
    let pattern = quote!(
        [T; {
            tokens!([Inner; ()]);
            1
        }]
    );
    let value = quote!(
        [u8; {
            tokens!([Inner; ()]);
            1
        }]
    );
    let (_, segments) = matched(quote!(([#pattern; ()],)), quote!((#value,))).unwrap();
    assert_eq!(segments[0].names, ["T"]);
    assert_eq!(capture(&segments[0].rows[0], "T"), "u8");
    let pattern = quote!(fn(#[opaque([Inner; ()])] label: T));
    let value = quote!(fn(#[opaque([Inner; ()])] other: u8));
    let (_, segments) = matched(quote!(([#pattern; ()],)), quote!((#value,))).unwrap();
    assert_eq!(segments[0].names, ["T"]);
    let (_, segments) =
        matched(quote!(([opaque!([Inner; ()]); ()],)), quote!((opaque!([Inner; ()]),))).unwrap();
    assert!(segments[0].names.is_empty());
    assert_eq!(segments[0].len(), 1);
}

#[test]
fn attachment_merge_is_idempotent_without_erasing_distinct_sibling_segments() {
    let (_, original) =
        matched(quote!((([Vec<T>; ()],), ([Vec<T>; ()],))), quote!(((Vec<u8>,), (Vec<u8>,))))
            .unwrap();
    let mut merged = Vec::new();
    merge_segments(&mut merged, original.clone());
    assert_eq!(merged.len(), 2);
    merge_segments(&mut merged, original);
    assert_eq!(merged.len(), 2);
    let (_, other) = matched(quote!(([Vec<T>; ()],)), quote!((Vec<u16>,))).unwrap();
    merge_segments(&mut merged, other);
    assert_eq!(merged.len(), 3);
    let (_, shifted) = matched(quote!((u8, [Vec<T>; ()])), quote!((u8, Vec<u8>))).unwrap();
    merge_segments(&mut merged, shifted);
    assert_eq!(merged.len(), 4);
}
