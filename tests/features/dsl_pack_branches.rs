//! Pack rows, candidate selection, and tuple power retain distinct boundaries.

use batch_impl::batch_impl;

struct Map<T, U>(T, U);
struct Pair<T, U>(T, U);

trait Same<T> {}
impl<T> Same<T> for T {}

#[batch_impl((*Map *().2 *().3,) where {
    @0_0: Same<u8>, @0_1: Same<u16>,
    @1_0: Same<u32>, @1_1: Same<u64>, @1_2: Same<u128>
})]
trait FlatGrid {}

#[batch_impl((*((),) (*Map *().2 *().3),))]
trait RowGrid {}

#[test]
fn two_axes_share_five_parameters_in_right_outer_order() {
    fn flat<T: FlatGrid>() {}
    fn rows<T: RowGrid>() {}
    flat::<(Map<u8, u32>, Map<u16, u32>, Map<u8, u64>, Map<u16, u64>, Map<u8, u128>, Map<u16, u128>)>(
    );
    rows::<(
        (Map<u8, u32>, Map<u16, u32>),
        (Map<u8, u64>, Map<u16, u64>),
        (Map<u8, u128>, Map<u16, u128>),
    )>();
}

#[batch_impl((*((),) (*Map *().1..=2 *().1..=3),))]
trait RowFamily {}

#[test]
fn row_shape_keeps_a_two_axis_length_family_coherent() {
    fn accepts<T: RowFamily>() {}
    accepts::<((Map<u8, u32>,),)>();
    accepts::<((Map<u8, u32>,), (Map<u8, u64>,))>();
    accepts::<((Map<u8, u32>,), (Map<u8, u64>,), (Map<u8, u128>,))>();
    accepts::<((Map<u8, u32>, Map<u16, u32>),)>();
    accepts::<((Map<u8, u32>, Map<u16, u32>), (Map<u8, u64>, Map<u16, u64>))>();
    accepts::<(
        (Map<u8, u32>, Map<u16, u32>),
        (Map<u8, u64>, Map<u16, u64>),
        (Map<u8, u128>, Map<u16, u128>),
    )>();
}

#[batch_impl(([*Vec, *Box] *().2,))]
trait UniformChoice {}

#[batch_impl((*([Vec, Box],) *().2,))]
trait LocalChoice {}

#[batch_impl((*(Vec, Box) *().2,))]
trait BothWrappers {}

#[test]
fn choices_select_whole_constructors_or_each_copied_slot() {
    fn uniform<T: UniformChoice>() {}
    fn local<T: LocalChoice>() {}
    fn both<T: BothWrappers>() {}
    uniform::<(Vec<u8>, Vec<u16>)>();
    uniform::<(Box<u8>, Box<u16>)>();
    local::<(Vec<u8>, Vec<u16>)>();
    local::<(Vec<u8>, Box<u16>)>();
    local::<(Box<u8>, Vec<u16>)>();
    local::<(Box<u8>, Box<u16>)>();
    both::<(Vec<u8>, Box<u8>, Vec<u16>, Box<u16>)>();
}

#[batch_impl((*Vec [*(u8,), *(u16, u32)],))]
trait BranchPacks {}

#[batch_impl((*Vec *(().1..=2),))]
trait CollectedFamily {}

#[test]
fn choice_families_and_explicit_collection_have_different_shapes() {
    fn branches<T: BranchPacks>() {}
    fn collected<T: CollectedFamily>() {}
    branches::<(Vec<u8>,)>();
    branches::<(Vec<u16>, Vec<u32>)>();
    collected::<(Vec<(u8,)>, Vec<(u16, u32)>)>();
}

#[batch_impl(Pair.*(self, Vec).().2)]
trait WholeTuple {}

#[test]
fn an_unpacked_tuple_is_one_complete_input_row() {
    fn accepts<T: WholeTuple>() {}
    accepts::<Pair<(u8, u16), Vec<(u8, u16)>>>();
}

#[batch_impl((*(u8, u16),).2)]
trait TupleCopiesPackSlot {}

#[batch_impl((*(),).2)]
trait TupleCopiesEmptySlot {}

#[batch_impl((*(*(),).2,))]
trait PackConsumesEmptySlot {}

#[batch_impl((*(().2),).0)]
trait ZeroCopiesDiscardSlot {}

#[batch_impl(([u8, u16],).2)]
trait TupleCopiesChoiceSlot {}

#[test]
fn tuple_power_does_not_preflatten_its_direct_slots() {
    fn copied<T: TupleCopiesPackSlot>() {}
    fn empty<T: TupleCopiesEmptySlot>() {}
    fn generated<T: PackConsumesEmptySlot>() {}
    fn discarded<T: ZeroCopiesDiscardSlot>() {}
    fn choice<T: TupleCopiesChoiceSlot>() {}
    copied::<(u8, u16, u8, u16)>();
    empty::<()>();
    generated::<(u8, u16)>();
    discarded::<()>();
    choice::<(u8, u8)>();
    choice::<(u8, u16)>();
    choice::<(u16, u8)>();
    choice::<(u16, u16)>();
}

struct First<A, B>(A, B);
struct Second<A, B>(A, B);
struct Third<A, B>(A, B);

#[batch_impl(*([*First, *Second],) *( *(u8, u16),))]
trait ChoiceInMapTask {}

#[batch_impl(*([*(First, Second), *Third],) *( *(u8, u16),))]
trait NestedChoiceInMapTask {}

#[test]
fn a_left_choice_does_not_split_a_complete_pack_row_again() {
    fn flat<T: ChoiceInMapTask>() {}
    fn nested<T: NestedChoiceInMapTask>() {}
    flat::<First<u8, u16>>();
    flat::<Second<u8, u16>>();
    nested::<First<u8, u16>>();
    nested::<Second<u8, u16>>();
    nested::<Third<u8, u16>>();
}

#[batch_impl((*(self, Vec) ([u8, u16],),))]
trait HiddenChoice {}

#[batch_impl((*(self, Vec) [(u8,), (u16,)],))]
trait ExposedChoice {}

#[test]
fn an_ordinary_tuple_preserves_hidden_independent_choices() {
    fn hidden<T: HiddenChoice>() {}
    fn exposed<T: ExposedChoice>() {}
    hidden::<((u8,), Vec<(u8,)>)>();
    hidden::<((u8,), Vec<(u16,)>)>();
    hidden::<((u16,), Vec<(u8,)>)>();
    hidden::<((u16,), Vec<(u16,)>)>();
    exposed::<((u8,), Vec<(u8,)>)>();
    exposed::<((u16,), Vec<(u16,)>)>();
}

#[batch_impl(Pair *[[u8, u16], [u32, u64]])]
trait NestedCandidates {}

#[batch_impl(Pair.*[u8, u16].2)]
trait ListPackPower {}

#[test]
fn nested_candidates_remain_independent_pack_slots() {
    fn nested<T: NestedCandidates>() {}
    fn power<T: ListPackPower>() {}
    nested::<Pair<u8, u32>>();
    nested::<Pair<u8, u64>>();
    nested::<Pair<u16, u32>>();
    nested::<Pair<u16, u64>>();
    power::<Pair<u8, u8>>();
    power::<Pair<u8, u16>>();
    power::<Pair<u16, u8>>();
    power::<Pair<u16, u16>>();
}
