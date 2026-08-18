//! dsl.rs entry-macro tests: `batch_trait!` (multi-segment, unsafe segment,
//! segment-level `@trait` bundles) and `batch_impl_only` (drops the trait
//! definition).
//! (split from the former single-file `tests/dsl.rs`)

use batch_impl::{batch_impl_only, batch_trait};
use std::collections::HashMap;

// ============================================================
// 16. batch_trait! function-like macro
// ============================================================
trait BTNumeric {}
trait BTMap {}

batch_trait!(
    BTNumeric: u8, u16, u32, u64;
    BTMap: HashMap<i32, i32>
);

#[test]
fn batch_trait_macro_basic() {
    fn check_num<T: BTNumeric>(_: &T) {}
    fn check_map<T: BTMap>(_: &T) {}
    check_num(&0u8);
    check_num(&0u16);
    check_num(&0u32);
    check_num(&0u64);
    check_map(&HashMap::<i32, i32>::new());
}

// ============================================================
// 17. batch_trait! multi-segment + unsafe segment
// ============================================================
trait PairSegment {}

batch_trait!(
    PairSegment: usize, isize;
    unsafe YieldUnsafe: u32
);

/// # Safety
///
/// Marker trait for testing; no actual unsafe semantics.
#[allow(dead_code)] // referenced via batch_trait!; the compiler does not see the impl
unsafe trait YieldUnsafe {}

#[test]
fn batch_trait_multi_segment_unsafe() {
    fn check_pair<T: PairSegment>(_: &T) {}
    check_pair(&0usize);
    check_pair(&0isize);
}

// ============================================================
// 18. batch_impl_only does not emit the trait definition
// ============================================================
trait DropDefOnly {
    fn m(&self) -> u32;
}

#[batch_impl_only(usize #m{42})]
trait DropDefOnly {
    fn m(&self) -> u32;
}

#[test]
fn batch_impl_only_drops_trait() {
    assert_eq!(0usize.m(), 42);
}

// ============================================================
// 34. batch_trait! segment-level @trait: reusing a "generic declaration + trait name" bundle
//     across segments (@trait inside constant values is replaced per segment with that
//     segment's trait path after entry splitting)
// ============================================================
trait SegA<T> {}
trait SegB<T> {}

batch_trait! {
    @type_t = <T> @trait <T>;
    SegA: @type_t [&, Box]^T;
    SegB: @type_t Box^[T, Vec<T>];
}

#[test]
fn trait_const_segment() {
    fn check_a<T: SegA<u8>>() {}
    fn check_b<T: SegB<u8>>() {}
    check_a::<&u8>();
    check_a::<Box<u8>>();
    check_b::<Box<u8>>();
    check_b::<Box<Vec<u8>>>();
}
