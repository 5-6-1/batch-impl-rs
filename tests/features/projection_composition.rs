//! Projection heads and tail arguments compose without applying to path names.

use batch_impl::batch_impl;

trait Family {
    type Out<T>;
    type Pair<A, B>;
    type Fixed;
    type Size<const N: usize>;
    type Borrowed<'a, T: 'a>;
}

impl Family for u8 {
    type Out<T> = (u8, T);
    type Pair<A, B> = (u8, A, B);
    type Fixed = u8;
    type Size<const N: usize> = [u8; N];
    type Borrowed<'a, T: 'a> = (u8, &'a T);
}

impl Family for u16 {
    type Out<T> = (u16, T);
    type Pair<A, B> = (u16, A, B);
    type Fixed = u16;
    type Size<const N: usize> = [u16; N];
    type Borrowed<'a, T: 'a> = (u16, &'a T);
}

#[batch_impl(<[u8, u16] as Family>::Out<[u32, u64]>)]
trait Matrix {}

#[test]
fn projection_heads_and_arguments_form_one_cartesian_matrix() {
    fn check<T: Matrix>() {}
    check::<(u8, u32)>();
    check::<(u8, u64)>();
    check::<(u16, u32)>();
    check::<(u16, u64)>();
}

#[batch_impl(<u8 as Family>::Out<Vec [u16, u32]>)]
trait AppliedArgument {}

#[test]
fn tail_arguments_use_ordinary_type_application() {
    fn check<T: AppliedArgument>() {}
    check::<(u8, Vec<u16>)>();
    check::<(u8, Vec<u32>)>();
}

#[batch_impl(<u8 as Family>::Pair<*[u16, u32]>)]
trait PackedArguments {}

#[batch_impl(<u8 as Family>::Out<(*Vec *[u16, u32],)>)]
trait CollectedArgument {}

#[test]
fn packs_splice_arguments_and_respect_inner_tuple_hosts() {
    fn packed<T: PackedArguments>() {}
    fn collected<T: CollectedArgument>() {}
    packed::<(u8, u16, u32)>();
    collected::<(u8, (Vec<u16>, Vec<u32>))>();
}

// Rust does not infer that GAT projections are injective. Trait arguments
// explicitly constrain the generated parameters independently of the target.
#[batch_impl(FreshArguments<@0, @1> <u8 as Family>::Pair<*[].2>)]
trait FreshArguments<A, B> {}

#[test]
fn fresh_arguments_hoist_declarations_and_keep_reference_identity() {
    fn check<A, B, T: FreshArguments<A, B>>() {}
    check::<String, bool, (u8, String, bool)>();
}

#[batch_impl(DeclaredArgument<T> <u8 as Family>::Out<(<T: Clone> T)> {
    fn copy(value: &T) -> T { value.clone() }
})]
trait DeclaredArgument<T> {
    fn copy(value: &T) -> T;
}

#[test]
fn declarations_inside_tail_arguments_reach_the_impl() {
    fn check<T: DeclaredArgument<String>>() {}
    check::<(u8, String)>();
    let value = String::from("bound preserved");
    assert_eq!(<(u8, String) as DeclaredArgument<String>>::copy(&value), value);
}

#[batch_impl(Synchronized<u32> <u8 as Family>::Out<>)]
trait Synchronized<T> {}

#[batch_impl(NestedSynchronized<u64> <u8 as Family>::Out<Vec<>>)]
trait NestedSynchronized<T> {}

#[batch_impl(BareSegment<u16> <u8 as Family>::Fixed)]
trait BareSegment<T> {}

#[test]
fn explicit_empty_arguments_sync_and_bare_segments_stay_bare() {
    fn direct<T: Synchronized<u32>>() {}
    fn nested<T: NestedSynchronized<u64>>() {}
    fn bare<T: BareSegment<u16>>() {}
    direct::<(u8, u32)>();
    nested::<(u8, Vec<u64>)>();
    bare::<u8>();
}

#[batch_impl(<'a, T: 'a> BorrowedArgument<'a, T> <u8 as Family>::Borrowed<'a, T>)]
trait BorrowedArgument<'a, T: 'a> {}

#[batch_impl(<u8 as Family>::Size<[2, 3]>)]
trait ConstArguments {}

#[test]
fn lifetime_and_const_arguments_keep_their_kinds() {
    fn borrowed<'a, T: 'a, U: BorrowedArgument<'a, T>>(_: U) {}
    fn constant<T: ConstArguments>() {}
    let value = String::from("borrowed");
    borrowed::<String, _>((1u8, &value));
    constant::<[u8; 2]>();
    constant::<[u8; 3]>();
}

#[batch_impl(<<[u8, u16] as Family>::Fixed as Family>::Out<[u32, u64]>)]
trait NestedProjection {}

#[test]
fn nested_projections_preserve_both_choice_axes() {
    fn check<T: NestedProjection>() {}
    check::<(u8, u32)>();
    check::<(u8, u64)>();
    check::<(u16, u32)>();
    check::<(u16, u64)>();
}

#[batch_impl(<u8 as Family>::Out<<u16 as Family>::Out<[u32, u64]>>)]
trait ProjectionArgument {}

#[test]
fn projection_arguments_expand_recursively() {
    fn check<T: ProjectionArgument>() {}
    check::<(u8, (u16, u32))>();
    check::<(u8, (u16, u64))>();
}

#[batch_impl(<u8 as Family>::Out<fn(
    #[allow(unused)] #[cfg_attr(any(), opaque(@99 <>))] value: Vec [u16, u32],
    #[allow(unused)] extra: bool,
) -> u64>)]
trait NamedFunctionArgument {}

#[test]
fn named_function_parameters_preserve_attributes_and_expand_types() {
    fn check<T: NamedFunctionArgument>() {}
    check::<(u8, fn(Vec<u16>, bool) -> u64)>();
    check::<(u8, fn(Vec<u32>, bool) -> u64)>();
}

#[batch_impl(<u8 as Family>::Out<[u16; [1usize, 2][0]]>)]
trait IndexedArrayLength {}

const ARRAY_LEN: &usize = &3;

#[batch_impl(<u8 as Family>::Out<[u16; *ARRAY_LEN]>)]
trait DereferencedArrayLength {}

#[test]
fn rust_array_lengths_are_not_classified_as_dsl_operators() {
    fn indexed<T: IndexedArrayLength>() {}
    fn dereferenced<T: DereferencedArrayLength>() {}
    indexed::<(u8, [u16; 1])>();
    dereferenced::<(u8, [u16; 3])>();
}
