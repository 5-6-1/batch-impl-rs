//! Body switches authorize lexical substitution, including macro input.

use batch_impl::batch_impl;

#[batch_impl(().2 impl{@0..} {
    // Deliberately repeats the same operand to preserve the reported example.
    #[allow(clippy::eq_op)]
    fn value() -> bool { !(@(false)||..) }
})]
trait Negated {
    fn value() -> bool;
}

#[test]
fn unary_not_does_not_hide_a_repeat_block() {
    assert!(<(u8, u16) as Negated>::value());
}

macro_rules! sum {
    ($($value:expr),* $(,)?) => { 0usize $(+ $value)* };
}

#[batch_impl(().2 impl{@0..} {
    fn counts() -> [usize; 4] {
        [sum!(@(1usize),..), sum![@(1usize),..], sum!{@(1usize),..},
         sum!(sum!(@(1usize),..))]
    }
})]
trait MacroRepeats {
    fn counts() -> [usize; 4];
}

#[batch_impl((u8, u16) impl{(A@..,)} {
    fn bytes() -> usize { sum!(@(::core::mem::size_of::<@A>()),..) }
})]
trait SegmentInMacro {
    fn bytes() -> usize;
}

#[test]
fn macro_groups_participate_in_repeat_and_driver_collection() {
    assert_eq!(<(u8, u16) as MacroRepeats>::counts(), [2; 4]);
    assert_eq!(<(u8, u16) as SegmentInMacro>::bytes(), 3);
}

macro_rules! sizes {
    ($($ty:ty),* $(,)?) => { 0usize $(+ ::core::mem::size_of::<$ty>())* };
}

#[batch_impl(().2 impl{@{}} {
    fn sizes() -> [usize; 4] {
        [sizes!(@{0}), sizes![@{1}], sizes!{@{0..}}, sizes!(@{0_0..})]
    }
})]
trait MacroFresh {
    fn sizes() -> [usize; 4];
}

#[test]
fn body_slot_switch_resolves_macro_fresh_inputs() {
    assert_eq!(<(u8, u16) as MacroFresh>::sizes(), [1, 2, 3, 3]);
}

#[batch_impl((u8,) impl{(A,)} {
    fn bytes() -> usize { sizes!(A) }
})]
trait FixedSlotInMacro {
    fn bytes() -> usize;
}

#[test]
fn fixed_shape_slots_remain_lexical_inside_macros() {
    assert_eq!(<(u8,) as FixedSlotInMacro>::bytes(), 1);
}

struct Wrapper<T>(T);

#[batch_impl(Wrapper<T>: Wrapper<().2>)]
impl Wrapper<T> {
    fn bytes() -> usize {
        fresh!(sum!(@(::core::mem::size_of::<@A>()),..))
    }
}

#[test]
fn impl_entry_fresh_marker_rewrites_nested_macro_inputs() {
    assert_eq!(Wrapper::<(u8, u16)>::bytes(), 3);
}

#[batch_impl(u8 {
    fn text() -> [&'static str; 3] {
        [stringify!(@trait), stringify!(@{0}), stringify!(@{999_0})]
    }
})]
trait Unswitched {
    fn text() -> [&'static str; 3];
}

#[test]
fn macro_markers_without_a_body_switch_remain_rust_input() {
    let actual = <u8 as Unswitched>::text().map(|s| s.replace(' ', ""));
    assert_eq!(actual, ["@trait", "@{0}", "@{999_0}"]);
}
