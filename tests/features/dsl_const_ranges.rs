//! Type-family ranges obey Rust endpoints without turning singletons into slices.

use batch_impl::{batch_impl, batch_trait};

#[batch_impl(@u8..u32 #tag{1})]
trait UnsignedExclusive {
    fn tag() -> u8;
}
// A handwritten endpoint impl proves that the range did not generate it.
impl UnsignedExclusive for u32 {
    fn tag() -> u8 {
        2
    }
}

#[batch_impl(@i8..i16 #tag{1})]
trait SignedExclusive {
    fn tag() -> u8;
}
impl SignedExclusive for i16 {
    fn tag() -> u8 {
        2
    }
}

#[batch_impl(@f32..f64 #tag{1})]
trait FloatExclusive {
    fn tag() -> u8;
}
impl FloatExclusive for f64 {
    fn tag() -> u8 {
        2
    }
}

#[test]
fn all_type_families_exclude_the_half_open_endpoint() {
    assert_eq!(<u8 as UnsignedExclusive>::tag(), 1);
    assert_eq!(<u16 as UnsignedExclusive>::tag(), 1);
    assert_eq!(<u32 as UnsignedExclusive>::tag(), 2);
    assert_eq!(<i8 as SignedExclusive>::tag(), 1);
    assert_eq!(<i16 as SignedExclusive>::tag(), 2);
    assert_eq!(<f32 as FloatExclusive>::tag(), 1);
    assert_eq!(<f64 as FloatExclusive>::tag(), 2);
}

#[batch_impl(@u8..=u16)]
trait Closed {}
#[batch_impl(@i16..=i16)]
trait ClosedSingleton {}
#[batch_impl(@..=u8)]
trait LeftClosedSingleton {}

#[test]
fn closed_endpoints_include_equal_bounds_as_a_single_type() {
    fn closed<T: Closed>() {}
    closed::<u8>();
    closed::<u16>();
    fn singleton<T: ClosedSingleton>() {}
    singleton::<i16>();
    fn left<T: LeftClosedSingleton>() {}
    left::<u8>();
}

#[batch_impl(@..u16 #tag{1})]
trait LeftExclusive {
    fn tag() -> u8;
}
impl LeftExclusive for u16 {
    fn tag() -> u8 {
        2
    }
}
#[batch_impl(@u128..)]
trait ThroughMaximum {}

#[test]
fn omitted_endpoints_keep_their_direction() {
    assert_eq!(<u8 as LeftExclusive>::tag(), 1);
    assert_eq!(<u16 as LeftExclusive>::tag(), 2);
    fn maximum<T: ThroughMaximum>() {}
    maximum::<u128>();
}

trait LazyRange {}
trait NestedRange {}
batch_trait! {
    @half = @u8..u16;
    @single = @u16..=u16;
    LazyRange: @half;
    NestedRange: Vec<@single>;
}
impl LazyRange for u16 {}

#[test]
fn user_constants_and_nested_arguments_preserve_range_kind() {
    fn lazy<T: LazyRange>() {}
    lazy::<u8>();
    lazy::<u16>();
    fn nested<T: NestedRange>() {}
    nested::<Vec<u16>>();
}

trait ImplRange {
    fn tag() -> u8;
}
#[batch_impl(@Self: Box<@u8..u16>)]
impl ImplRange for Box<u8> {
    fn tag() -> u8 {
        1
    }
}
impl ImplRange for Box<u16> {
    fn tag() -> u8 {
        2
    }
}

#[test]
fn impl_entry_uses_the_same_endpoint_rules() {
    assert_eq!(<Box<u8> as ImplRange>::tag(), 1);
    assert_eq!(<Box<u16> as ImplRange>::tag(), 2);
}
