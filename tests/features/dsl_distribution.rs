//! dsl.rs list-distribution tests: array dispatch in nested positions
//! (tuples / generic args) and pow_cartesian output nested in outer tuples.
//! (split from the former single-file `tests/dsl.rs`)

use batch_impl::batch_impl;

// ---- List distribution in nested positions (0.7.0) ----

#[batch_impl((u8, [u16, u32, u64]))]
trait TupDist {}

#[batch_impl(([u8, u16], [u32, u64]))]
trait TupDist2 {}

#[batch_impl(Vec<[u8, u16, u32]>)]
trait GenDist {}

#[batch_impl(Box<[u8, u16]>)]
trait TraitDist {}

#[test]
fn list_distribution_nested() {
    fn assert_t1<T: TupDist>() {}
    assert_t1::<(u8, u16)>();
    assert_t1::<(u8, u64)>();
    fn assert_t2<T: TupDist2>() {}
    assert_t2::<(u8, u32)>();
    assert_t2::<(u16, u64)>();
    fn assert_g<T: GenDist>() {}
    assert_g::<Vec<u8>>();
    assert_g::<Vec<u32>>();
    fn assert_t<T: TraitDist>() {}
    assert_t::<Box<u8>>();
    assert_t::<Box<u16>>();
}

// pow_cartesian output nested in an outer tuple: the array of combos must
// distribute through the tuple (user scenario `(()^2, ((A_,)^2,(<Clone>,)^2)^3, ()^4)^2`).
// Fresh counts differ per generator (`()^2` / `()^3`) so no combo pair
// overlaps after the sweep rename (E0119 is the user's responsibility when
// concrete and fresh generators collide).
struct A_;
#[batch_impl(((A_,)^2, ((A_,)^2,(<Clone>,)^2)^2, ()^3)^2)]
trait NestedPow {}

#[test]
fn pow_cartesian_nested_in_tuple() {
    fn assert_t<T: NestedPow>() {}
    // `[e0, e0]` — both positions pick the `(A_,)^2` generator
    assert_t::<((A_, A_), (A_, A_))>();
    // `[e0, e2]` — position 2 picks the `()^3` fresh trio
    assert_t::<((A_, A_), (u8, u16, u32))>();
    // `[e0, e1_1]` — position 2 picks an inner cartesian combo
    // (`((A_,)^2, (<Clone>,)^2)` combo 2 = `(A_,A_), (C0,C1)`)
    assert_t::<((A_, A_), ((A_, A_), (u8, u16)))>();
}
