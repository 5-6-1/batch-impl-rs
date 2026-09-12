//! regression.rs §1-6: high-value corner cases — nested angle brackets,
//! path types, const generics, mixed type+const generics, lifetime generics,
//! dyn traits with multiple bounds.
//! (split from the former single-file `tests/regression.rs`)

use batch_impl::batch_impl;

// ============================================================
// 1. Nested angle brackets Vec<Vec<T>> — verify `>>` does not break depth tracking
// ============================================================
#[batch_impl(<T> Vec<Vec<T>>)]
trait NestedGeneric {}

#[test]
fn nested_angle_brackets() {
    fn _check<T: NestedGeneric>() {}
    _check::<Vec<Vec<i32>>>();
    _check::<Vec<Vec<String>>>();
}

// ============================================================
// 2. Path type std::collections::HashMap<K, V>
// ============================================================
#[batch_impl(<K, V> std::collections::HashMap<K, V>)]
trait PathType {}

#[test]
fn path_type_with_generics() {
    fn _check<T: PathType>() {}
    _check::<std::collections::HashMap<i32, String>>();
}

// ============================================================
// 3. const generic <const N: usize> [i32; N]
// ============================================================
#[batch_impl(<const N: usize> ConstGeneric<N> [i32; N] {
    fn len_const(&self) -> usize { N }
    fn first(&self) -> i32 { self[0] }
})]
trait ConstGeneric<const N: usize> {
    fn len_const(&self) -> usize;
    fn first(&self) -> i32;
}

#[test]
fn const_generic_array() {
    let arr: [i32; 5] = [10, 20, 30, 40, 50];
    assert_eq!(arr.len_const(), 5);
    assert_eq!(arr.first(), 10);
}

// ============================================================
// 4. Type annotations mixed with const generics <T: Clone, const N: usize>
//    Verify the DSL is not confused by spaces / commas in `<T : Clone , const N : usize>`
// ============================================================
#[batch_impl(<T: Clone, const N: usize> MixedGeneric<T, N> [T; N] {
    fn repeat_inner(&self) -> Vec<T> {
        std::iter::repeat_n(self[0].clone(), N).collect()
    }
})]
trait MixedGeneric<T, const N: usize> {
    fn repeat_inner(&self) -> Vec<T>;
}

#[test]
fn mixed_type_bound_and_const_generic() {
    let arr: [String; 3] = [String::from("hi"), String::from("hi"), String::from("hi")];
    assert_eq!(arr.repeat_inner().len(), 3);
}

// ============================================================
// 5. Lifetime generics <'a, T: 'a> &'a T
// ============================================================
#[allow(dead_code)]
#[batch_impl(<'a, T: 'a> LifetimeTrait<'a, T> &'a T)]
trait LifetimeTrait<'a, T> {}

#[test]
fn lifetime_generic() {
    fn _check<'a, T: 'a>()
    where
        &'a T: LifetimeTrait<'a, T>,
    {
    }
    _check::<'static, i32>();
}

// ============================================================
// 6. dyn trait + multiple bounds (`+ Send + Sync`)
// ============================================================
#[batch_impl(dyn std::fmt::Display + Send + Sync)]
trait DynMarkerMultiBound {}

#[test]
fn dyn_trait_with_multi_bounds() {
    fn _check<T: DynMarkerMultiBound + ?Sized>() {}
    _check::<dyn std::fmt::Display + Send + Sync>();
}

// ============================================================
// 7. `X<>` on the **target** must not corrupt the spec's trait args
// ============================================================
// `A<>` at the spec **head** is the "declare the trait's formals here" shorthand
// (`dsl_generics.rs`'s `EmptyGenA/B/C`). The same marker further along the spec —
// on the target — used to be rewritten too, and the declaration block it emitted
// landed between the trait head and the target, which the parse merged into the
// trait's own arguments: `impl<T> TargetMarker<T, T>` (measured; an impl nobody
// can compile). Only the head expands now, and the target's marker is filled by
// the codegen sync (`X<>` → the spec's args).
struct Swap2<A>(A);

#[batch_impl(<T> TargetMarker<T> Swap2<> { fn tag(&self) -> u8 { 0 } })]
trait TargetMarker<T> {
    fn tag(&self) -> u8;
}

#[test]
fn a_target_position_marker_fills_instead_of_duplicating_the_trait_args() {
    // `TargetMarker<u8>` for `Swap2<u8>`: the trait takes exactly one argument,
    // so the old `TargetMarker<T, T>` rendering could not even name this impl.
    fn need<X: TargetMarker<u8>>() {}
    need::<Swap2<u8>>();
    assert_eq!(Swap2(7u8).tag(), 0);
}
