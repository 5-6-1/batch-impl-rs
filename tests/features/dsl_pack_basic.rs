//! Public Pack constructors, per-position mapping, and fresh sharing.

use batch_impl::batch_impl;

struct Pair<A, B>(A, B);

#[batch_impl(Pair *(u8, u16))]
trait Arguments {}

#[batch_impl((*Vec *(u8, u16),))]
trait ConcreteMap {}

#[test]
fn an_ordinary_constructor_consumes_arguments_and_a_pack_maps() {
    fn arguments<T: Arguments>() {}
    fn mapped<T: ConcreteMap>() {}
    arguments::<Pair<u8, u16>>();
    mapped::<(Vec<u8>, Vec<u16>)>();
}

#[batch_impl((*Vec *().3,) #width { 3 })]
trait FixedVectors {
    fn width() -> usize;
}

#[batch_impl((*Vec *().1..=3,))]
trait VectorFamily {}

#[test]
fn each_fresh_position_is_wrapped_and_range_lengths_stay_separate() {
    assert_eq!(<(Vec<u8>, Vec<u16>, Vec<u32>) as FixedVectors>::width(), 3);
    fn family<T: VectorFamily>() {}
    family::<(Vec<u8>,)>();
    family::<(Vec<u8>, Vec<u16>)>();
    family::<(Vec<u8>, Vec<u16>, Vec<u32>)>();
}

#[batch_impl((*Pair (*(self, Vec) *().3),))]
trait FixedPairs {}

#[batch_impl((*Pair.*(self, Vec).*().1..=3,))]
trait PairFamily {}

#[test]
fn composed_constructors_reuse_each_positions_fresh_identity() {
    fn fixed<T: FixedPairs>() {}
    fn family<T: PairFamily>() {}
    fixed::<(Pair<u8, Vec<u8>>, Pair<u16, Vec<u16>>, Pair<u32, Vec<u32>>)>();
    family::<(Pair<u8, Vec<u8>>,)>();
    family::<(Pair<u8, Vec<u8>>, Pair<u16, Vec<u16>>)>();
    family::<(Pair<u8, Vec<u8>>, Pair<u16, Vec<u16>>, Pair<u32, Vec<u32>>)>();
}

#[batch_impl((*((),) (*(self, Vec) *().3),))]
trait TuplePerPosition {}

#[batch_impl((*(self, Vec) *().2,))]
trait FlatMembers {}

#[test]
fn an_inner_tuple_is_an_explicit_host() {
    fn nested<T: TuplePerPosition>() {}
    fn flat<T: FlatMembers>() {}
    nested::<((u8, Vec<u8>), (u16, Vec<u16>), (u32, Vec<u32>))>();
    flat::<(u8, Vec<u8>, u16, Vec<u16>)>();
}

#[batch_impl((*((),) (*(Vec, Vec) *().2),))]
trait PairedBuffers {}

#[test]
fn two_buffers_for_each_position_share_one_type_parameter() {
    fn accepts<T: PairedBuffers>() {}
    accepts::<((Vec<u8>, Vec<u8>), (Vec<u16>, Vec<u16>))>();
}

#[batch_impl((*Vec (*Box *().2),))]
trait NestedWrap {}

#[test]
fn nested_application_builds_each_inner_wrapper_before_the_outer_one() {
    fn accepts<T: NestedWrap>() {}
    accepts::<(Vec<Box<u8>>, Vec<Box<u16>>)>();
}

#[batch_impl((*().0,))]
trait EmptyTuple {}

#[batch_impl(*Vec *())]
trait NoTargets {}

// This blanket remains coherent only if the empty map emits no impls.
impl<T> NoTargets for T {}

#[batch_impl(*u8 *(*(),))]
trait EmptyRow {}

#[test]
fn empty_packs_and_a_pack_with_one_empty_row_are_distinct() {
    fn empty<T: EmptyTuple>() {}
    fn row<T: EmptyRow>() {}
    empty::<()>();
    row::<u8>();
    fn no_targets<T: NoTargets>() {}
    no_targets::<u8>();
}

#[batch_impl((*(*u8),))]
trait Idempotent {}

#[batch_impl((*Pair *( *(u8, u16),),))]
trait PairRow {}

#[test]
fn nested_stars_are_idempotent_but_nested_pack_rows_are_structural() {
    fn scalar<T: Idempotent>() {}
    fn row<T: PairRow>() {}
    scalar::<(u8,)>();
    row::<(Pair<u8, u16>,)>();
}

#[batch_impl((**const u8,))]
trait PointerOperand {}

#[test]
fn raw_pointer_parsing_precedes_the_pack_prefix() {
    fn accepts<T: PointerOperand>() {}
    accepts::<(*const u8,)>();
}
