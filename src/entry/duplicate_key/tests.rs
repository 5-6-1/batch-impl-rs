//! Head comparison, namespace and resource-boundary regressions.

use super::key;

fn head(source: &str) -> String {
    key(&syn::parse_str(source).unwrap()).unwrap_or_else(|| panic!("unsupported head: {source}"))
}

#[test]
fn nested_generic_keys_stay_small_without_recursive_escaping() {
    let mut previous_length = 0;
    for depth in [20, 40] {
        // Parse only shallow shells: deeply recursive syn parsing can exhaust
        // the Windows test thread's stack before the key itself is exercised.
        let mut ty: syn::Type = syn::parse_quote!(u8);
        for _ in 0..depth {
            let mut outer: syn::TypePath = syn::parse_quote!(Box<()>);
            let syn::PathArguments::AngleBracketed(args) =
                &mut outer.path.segments.last_mut().unwrap().arguments
            else {
                panic!("expected generic shell");
            };
            args.args = std::iter::once(syn::GenericArgument::Type(ty)).collect();
            ty = syn::Type::Path(outer);
        }
        let mut item: syn::ItemImpl = syn::parse_quote!(impl Tr for () {});
        *item.self_ty = ty;
        let normalized = key(&item).unwrap();
        assert!(normalized.len() < 160 * depth, "depth {depth}: {} bytes", normalized.len());
        if previous_length > 0 {
            assert!(normalized.len() < 3 * previous_length);
        }
        previous_length = normalized.len();
    }
}

#[test]
fn length_prefixes_keep_arbitrary_field_boundaries_distinct() {
    use super::node;
    assert_ne!(node("n", vec!["a".into(), "bc".into()]), node("n", vec!["ab".into(), "c".into()]));
    assert_ne!(node("n", vec![]), node("n", vec![String::new()]));
    assert_ne!(node("n", vec!["1:a".into()]), node("n", vec!["a".into()]));
    assert_ne!(node("n", vec!["x:1:1:y".into()]), node("x", vec!["n:1:1:y".into()]));
    let field = "é:\\\"".to_string();
    assert_eq!(node("n", vec![field.clone()]), format!("n:1:{}:{field}", field.len()));
}

#[test]
fn ignores_body_but_preserves_full_trait_path_arguments_and_self_type() {
    let plain = head("impl crate::Tr<u8> for Vec<u16> {}");
    assert_eq!(plain, head("impl crate::Tr<u8> for Vec<u16> { fn f() {} }"));
    for other in [
        "impl other::Tr<u8> for Vec<u16> {}",
        "impl crate::Tr<u16> for Vec<u16> {}",
        "impl crate::Tr<u8> for Vec<u32> {}",
        "unsafe impl crate::Tr<u8> for Vec<u16> {}",
    ] {
        assert_ne!(plain, head(other), "{other}");
    }
}

#[test]
fn first_use_normalizes_permuted_fresh_declarations_including_unused_ones() {
    assert_eq!(
        head("impl<P0, P1> Tr for Pair<P0, Vec<P0>> {}"),
        head("impl<P0, P1> Tr for Pair<P1, Vec<P1>> {}"),
    );
    assert_eq!(
        head("impl<A, B> Tr<A> for Pair<B, Vec<A>> {}"),
        head("impl<Y, X> Tr<X> for Pair<Y, Vec<X>> {}"),
    );
    assert_ne!(
        head("impl<A, B> Tr for Pair<A, Vec<A>> {}"),
        head("impl<A, B> Tr for Pair<A, Vec<B>> {}"),
    );
}

#[test]
fn preserves_declarations_bounds_and_where_predicates_after_renaming() {
    assert_eq!(
        head("impl<A: Clone, B: Copy> Tr for Pair<A, B> where A: Into<B>, B: Send {}"),
        head("impl<Y: Copy, X: Clone> Tr for Pair<X, Y> where Y: Send, X: Into<Y> {}"),
    );
    assert_eq!(
        head("impl<A: Send + Clone> Tr for A {}"),
        head("impl<B: Clone + Send> Tr for B {}"),
    );
    let original = head("impl<A, B: Clone> Tr for A {}");
    assert_ne!(original, head("impl<A, B: Copy> Tr for A {}"));
    assert_ne!(original, head("impl<A> Tr for A {}"));
    assert_ne!(head("impl<A: Clone> Tr for A {}"), head("impl<A: Copy> Tr for A {}"));
    assert_ne!(
        head("impl<A> Tr for A where A: Clone {}"),
        head("impl<A> Tr for A where A: Copy {}"),
    );
}

#[test]
fn generic_roots_do_not_rename_path_segments_or_associated_keys() {
    assert_eq!(
        head("impl<T> Tr for Pair<T, T::Item> {}"),
        head("impl<U> Tr for Pair<U, U::Item> {}"),
    );
    assert_ne!(
        head("impl<T> Tr for Pair<T, module::T> {}"),
        head("impl<U> Tr for Pair<U, module::U> {}"),
    );
    assert_ne!(head("impl<T> Tr for Pair<T, ::T> {}"), head("impl<U> Tr for Pair<U, ::U> {}"),);
    assert_ne!(
        head("impl<T> Tr for T where T: Iterator<T = u8> {}"),
        head("impl<U> Tr for U where U: Iterator<U = u8> {}"),
    );
    assert_eq!(
        head("impl<T> Tr for T where T: Iterator<Item = T> {}"),
        head("impl<U> Tr for U where U: Iterator<Item = U> {}"),
    );
}

#[test]
fn qualified_projection_only_renames_its_self_and_argument_positions() {
    assert_eq!(
        head("impl<T> Tr for <T as Other<T>>::Item {}"),
        head("impl<U> Tr for <U as Other<U>>::Item {}"),
    );
    assert_ne!(
        head("impl<T> Tr for <T as module::T>::Item {}"),
        head("impl<U> Tr for <U as module::U>::Item {}"),
    );
    assert_ne!(
        head("impl<T> Tr for <T as Other>::T {}"),
        head("impl<U> Tr for <U as Other>::U {}"),
    );
}

#[test]
fn lifetime_and_const_binders_preserve_kind_type_and_relationships() {
    assert_eq!(
        head("impl<'a, T, const N: usize> Tr<'a> for (&'a T, [T; N]) where T: 'a {}"),
        head("impl<'b, const M: usize, U> Tr<'b> for (&'b U, [U; M]) where U: 'b {}"),
    );
    assert_ne!(
        head("impl<const N: usize> Tr for [u8; N] {}"),
        head("impl<const N: u8> Tr for [u8; N] {}"),
    );
    assert!(
        key(&syn::parse_quote!(
            impl<const T: usize> Tr for Array<T> {}
        ))
        .is_none()
    );
    assert_ne!(
        head("impl<'a, 'b> Tr for (&'a u8, &'a u8) {}"),
        head("impl<'a, 'b> Tr for (&'a u8, &'b u8) {}"),
    );
}

#[test]
fn type_and_value_namespaces_do_not_share_binder_substitution() {
    // Global value constants T/U coexist with same-name type parameters.
    assert_ne!(head("impl<T> Tr<T> for [u8; T] {}"), head("impl<U> Tr<U> for [u8; U] {}"),);
    // Global types N/M coexist with same-name const parameters.
    assert_ne!(
        head("impl<const N: usize> Tr for (N, [u8; N]) {}"),
        head("impl<const M: usize> Tr for (M, [u8; M]) {}"),
    );
    assert_ne!(
        head("impl<const N: usize> Tr for ([u8; N::SIZE], [u8; N]) {}"),
        head("impl<const M: usize> Tr for ([u8; M::SIZE], [u8; M]) {}"),
    );
    assert_eq!(
        head("impl<T> Tr<T> for [u8; T::SIZE] {}"),
        head("impl<U> Tr<U> for [u8; U::SIZE] {}"),
    );
    assert_eq!(
        head("impl<const N: usize> Tr for [u8; N] {}"),
        head("impl<const M: usize> Tr for [u8; M] {}"),
    );
    // Bare N in a generic argument may name the global type or the const
    // binder. External parameter kinds are not available to this checker.
    for source in [
        "impl<const N: usize> Tr<N> for [u8; N] {}",
        "impl<const M: usize> Tr<M> for [u8; M] {}",
        "impl<const N: usize> Tr for Array<N> {}",
    ] {
        assert!(key(&syn::parse_str(source).unwrap()).is_none(), "{source}");
    }
}

#[test]
fn structural_hosts_and_raw_identifiers_have_unambiguous_keys() {
    assert_eq!(
        head("impl<T> Tr for (fn(T), *const T, &'static [T], [T; 3]) {}"),
        head("impl<U> Tr for (fn(U) -> (), *const U, &'static [U], [U; 3]) {}"),
    );
    assert_eq!(head("impl<r#T> Tr for r#T {}"), head("impl<U> Tr for U {}"));
    assert_eq!(head("impl Tr for r#Box<u8> {}"), head("impl Tr for Box<u8> {}"));
    assert_ne!(head("impl Tr for [u8; 3] {}"), head("impl Tr for [u8; 4] {}"));
    assert_ne!(head("impl Tr for *const u8 {}"), head("impl Tr for *mut u8 {}"));
    assert_ne!(head("impl Tr for fn() {}"), head("impl Tr for unsafe fn() {}"));
    assert_ne!(head("impl Tr for (u8,) {}"), head("impl Tr for (u8) {}"));
    assert_ne!(head("impl Tr for T0 {}"), head("impl<T> Tr for T {}"));
}

#[test]
fn unsupported_or_conditional_heads_are_skipped_without_looking_at_bodies() {
    for source in [
        "impl Target {}",
        "#[cfg(any())] impl Tr for u8 {}",
        "impl<#[cfg(any())] T> Tr for T {}",
        "impl<T = u8> Tr for T {}",
        "impl<const N: usize = 1> Tr for Array<N> {}",
        "impl Tr for make_type!() {}",
        "impl Tr for [u8; { 3 }] {}",
        "impl<const N: usize> Tr for [u8; N + 1] {}",
        "impl Tr for for<'a> fn(&'a u8) {}",
        "impl<T> Tr for T where T: for<'a> Other<'a> {}",
        "impl<T> Tr for T where for<'a> &'a T: Other {}",
        "impl Tr for dyn Other {}",
        "impl Tr for impl Other {}",
        "impl<T> Tr for T where T: Other<Item<'static> = u8> {}",
    ] {
        let item = syn::parse_str(source).unwrap_or_else(|e| panic!("{source}: {e}"));
        assert!(key(&item).is_none(), "{source}");
    }
    assert_eq!(
        head("impl Tr for u8 {}"),
        head("impl Tr for u8 { #[cfg(any())] fn f() { opaque!(); } }"),
    );
}
