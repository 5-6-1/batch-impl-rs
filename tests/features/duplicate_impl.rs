//! Definite-duplicate checks preserve distinct heads and opaque Rust behavior.

use batch_impl::{batch_impl, batch_impl_only, batch_trait};

#[batch_impl(#[cfg(any())] u8, u8)]
trait Configured {}

#[batch_impl(#[cfg_attr(all(), cfg(any()))] u16, u16)]
trait ConditionallyConfigured {}

#[batch_impl(u32 { #![cfg(any())] }, u32)]
trait InnerConfigured {}

#[test]
fn configuration_can_remove_one_of_two_equal_heads() {
    fn direct<T: Configured>() {}
    fn conditional<T: ConditionallyConfigured>() {}
    fn inner<T: InnerConfigured>() {}
    direct::<u8>();
    conditional::<u16>();
    inner::<u32>();
}

// Only the first generated impl has this attribute. It changes that impl's
// target to u16; treating the two initial u8 heads as duplicates would be wrong.
#[batch_impl(#[batch_impl(@Self: u16)] u8, u8)]
trait RewrittenByAttribute {}

#[test]
fn an_attribute_can_change_a_generated_impl_head() {
    fn accepts<T: RewrittenByAttribute>() {}
    accepts::<u8>();
    accepts::<u16>();
}

#[batch_impl(Argument<u8> u32, Argument<u16> u32)]
trait Argument<T> {}

trait OnlyArgument<T> {}

#[batch_impl_only(OnlyArgument<u8> u64, OnlyArgument<u16> u64)]
trait OnlyArgument<T> {}

trait FirstSegment {}
trait SecondSegment {}

batch_trait! {
    FirstSegment: u8;
    SecondSegment: u8;
}

#[test]
fn trait_paths_and_arguments_are_part_of_the_impl_head() {
    fn arguments<T: Argument<u8> + Argument<u16>>() {}
    fn only<T: OnlyArgument<u8> + OnlyArgument<u16>>() {}
    fn segments<T: FirstSegment + SecondSegment>() {}
    arguments::<u32>();
    only::<u64>();
    segments::<u8>();
}

struct Inherent;

#[batch_impl(method: [first, second])]
impl Inherent {
    fn method(&self) -> u8 {
        7
    }
}

#[test]
fn inherent_impls_can_share_the_same_type_with_different_methods() {
    assert_eq!(Inherent.first(), 7);
    assert_eq!(Inherent.second(), 7);
}

struct Pair<T, U>(T, U);

#[batch_impl((*Pair.*[self, Vec].*[].2,))]
trait CollectedPairs {}

#[test]
fn dot_mapping_collects_one_target_with_independent_positions() {
    fn accepts<T: CollectedPairs>() {}
    accepts::<(Pair<u8, Vec<u8>>, Pair<u16, Vec<u16>>)>();
}

#[batch_impl((*[u8, u8],), Pair<*[u8, u8]>)]
trait RepeatedMembers {}

#[test]
fn repeated_pack_members_remain_valid_inside_one_target() {
    fn accepts<T: RepeatedMembers>() {}
    accepts::<(u8, u8)>();
    accepts::<Pair<u8, u8>>();
}

macro_rules! opaque_type {
    (A) => {
        u8
    };
    (B) => {
        u16
    };
}

// The token in a macro invocation is user syntax, not a generic reference
// that alpha-normalization may rename. These two macros return different types.
#[batch_impl(<A> (A, opaque_type!(A)), <B> (B, opaque_type!(B)))]
trait OpaqueType {}

macro_rules! opaque_length {
    (N) => {
        1
    };
    (M) => {
        2
    };
}

struct ArrayMarker<const N: usize, const K: usize>;

#[batch_impl(
    <const N: usize> ArrayMarker<N, { opaque_length!(N) }>,
    <const M: usize> ArrayMarker<M, { opaque_length!(M) }>
)]
trait OpaqueConst {}

#[test]
fn macro_tokens_in_types_and_const_arguments_are_not_generic_references() {
    fn ty<T: OpaqueType>() {}
    fn constant<T: OpaqueConst>() {}
    ty::<(bool, u8)>();
    ty::<(char, u16)>();
    constant::<ArrayMarker<3, 1>>();
    constant::<ArrayMarker<4, 2>>();
}

mod namespace_scopes {
    use batch_impl::batch_impl;

    const T: usize = 1;
    const U: usize = 2;

    // A type parameter does not shadow a constant in an array-length expression.
    #[batch_impl(
        <T> TypeParameterLengths<T> [u8; T],
        <U> TypeParameterLengths<U> [u8; U]
    )]
    trait TypeParameterLengths<A> {}

    #[test]
    fn type_parameter_names_do_not_rename_value_namespace_constants() {
        fn one<X: TypeParameterLengths<bool>>() {}
        fn two<X: TypeParameterLengths<char>>() {}
        one::<[u8; 1]>();
        two::<[u8; 2]>();
    }

    struct N;
    struct M;

    // These trait arguments name the global types; only the array lengths
    // refer to the const parameters with the same spellings.
    #[batch_impl(
        <const N: usize> ConstParameterTypes<N> [u8; N],
        <const M: usize> ConstParameterTypes<M> [u8; M]
    )]
    trait ConstParameterTypes<A> {}

    #[test]
    fn const_parameter_names_do_not_rename_type_namespace_items() {
        fn n<X: ConstParameterTypes<N>>() {}
        fn m<X: ConstParameterTypes<M>>() {}
        n::<[u8; 3]>();
        m::<[u8; 4]>();
    }
}
