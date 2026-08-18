//! dsl.rs `@N` / `@g_i` / `@all_fresh` / `@N..M` position-reference tests:
//! document-order numbering, per-impl sweeping, group-position references,
//! references inside the target type, and the batch where-references.
//! (split from the former single-file `tests/dsl.rs`)

use batch_impl::batch_impl;

// @0 generalization: positional references usable in ordinary where predicates
// (tuple-generated generics / user generics)
#[batch_impl(()^2 where{@0: Clone, @1: Copy} { fn tmk() -> u32 { 2 } })]
trait TupleWhereAt {
    fn tmk() -> u32;
}

#[batch_impl(<T> AtWhere<T> Vec<T> where{T: Default} { fn an(&self) -> usize { self.len() } })]
trait AtWhere<T: Clone> {
    fn an(&self) -> usize;
}

// @N is matched by number (= generation order = the target type's document
// order), not by declaration order: in a join the declaration order differs
// from the document order (`()^3-()^3` declares the nested tuple first), so
// `@0` must still be the first fresh as it appears in the target type. The
// `JoinMarker` bound (implemented only for u8) verifies `@0` is the document-
// order first fresh (u8) — the old declaration-order indexing would resolve
// `@0` to the nested tuple's first element (u64) and fail to compile.
trait JoinMarker {}
impl JoinMarker for u8 {}

#[batch_impl(()^3-()^3 where{@0: JoinMarker, @5: Copy})]
trait JoinAtNum {}

#[test]
fn at_refs_numbered_match_in_join() {
    // Trigger instantiation of the generated impl (its where clause checks
    // `_Param_0_: JoinMarker` and `_Param_5_: Copy` against the concrete type).
    fn assert_impl<T: JoinAtNum>() {}
    assert_impl::<(u8, u16, u32, (u64, u128, usize))>();
}

// @all_fresh: every fresh generic gets the predicate tail (comma-separated)
#[batch_impl(()^2-()^2 where{@all_fresh: Clone})]
trait AllFreshWhere {}

// @N..=M: contiguous fresh range — `@0..=1` bounds the first two freshes
#[batch_impl(()^2-()^2 where{@0..=1: Copy})]
trait RangeWhere {}

#[test]
fn at_all_fresh_and_range() {
    // `()^2-()^2` targets `(A, B, (C, D))` (left tuple flattened, right
    // nested). @all_fresh: all 4 fresh generics (swept 0..4) must be Clone.
    fn assert_impl_all<T: AllFreshWhere>() {}
    assert_impl_all::<(u8, u16, (u32, u64))>();
    // @0..=1: only the first two freshes (swept `_Param_0_`, `_Param_1_` —
    // A and B) must be Copy; C, D are unconstrained (String / Vec are not
    // Copy — if the range leaked past `=1` this would fail to compile)
    fn assert_impl_range<T: RangeWhere>() {}
    assert_impl_range::<(u8, u16, (String, Vec<u8>))>();
}

// @all_fresh and @N..=M in the *same* where group: the group is split into
// predicates at depth-0 commas so the @all_fresh expansion must not swallow
// the following @N..=M predicate.
#[batch_impl(()^3-()^3 where{@all_fresh: Clone, @0..=2: Copy})]
trait CombinedBatchWhere {}

#[test]
fn at_all_fresh_with_range_same_group() {
    fn assert_impl<T: CombinedBatchWhere>() {}
    // all 6 freshes Clone + first 3 Copy (u128/usize are Clone; not Copy-bound)
    assert_impl::<(u8, u16, u32, (u64, u128, usize))>();
}

// Fresh names are swept per impl to `_Param_0..N_BatchGen_` (grouped
// `_Param_{g}_{i}_` generation → document-order renumber), so `@N` is a pure
// construction that works across generation units: a range spec generates one
// impl per length, each sweeping its own fresh to 0..N — `@0` is "this impl's
// first" in every length (previously the numbering drifted across lengths and
// `@0` errored on the later impls).
#[batch_impl(()^1..=3 where{@0: Clone} { fn tmk() -> u32 { 2 } })]
trait RangeAtNum {
    fn tmk() -> u32;
}

// Same for multiple specs: each spec sweeps independently, so spec 2's `@0`
// is spec 2's first fresh (previously the counter continued across specs).
#[batch_impl(()^2, ()^3 where{@0: Clone})]
trait MultiAtNum {}

#[test]
fn at_refs_across_generation_units() {
    assert_eq!(<(u8,) as RangeAtNum>::tmk(), 2);
    assert_eq!(<(u8, u16) as RangeAtNum>::tmk(), 2);
    assert_eq!(<(u8, u16, u32) as RangeAtNum>::tmk(), 2);
    // `@0: Clone` must resolve in *both* specs (each sweeps its own fresh).
    fn assert_impl<T: MultiAtNum>() {}
    assert_impl::<(u8, u16)>();
    assert_impl::<(u8, u16, u32)>();
}

// `@g_i` structured references: group g, position i of the generating site.
// In `()^3-()^3` the left generator is group 0, the right is group 1 — `@0_0`
// is the left group's first fresh (A = u8: JoinMarker) and `@1_0` the right
// group's first (D = u64: Copy). Unlike `@N`, `@g_i` is stable across
// array-dispatch impls (a group absent from an impl errors instead of
// silently shifting).
#[batch_impl(()^3-()^3 where{@0_0: JoinMarker, @1_0: Copy})]
trait JoinAtGroup {}

#[test]
fn at_group_position_refs() {
    fn assert_impl<T: JoinAtGroup>() {}
    assert_impl::<(u8, u16, u32, (u64, u128, usize))>();
}

#[test]
fn where_position_refs() {
    assert_eq!(<(u32, u32) as TupleWhereAt>::tmk(), 2);
    let v = vec![1u32];
    assert_eq!(v.an(), 1);
}

// `@N` / `@g_i` in the target type: `(()^2)^Box<@0>` appends the boxed
// reference to the generated tuple, so the reference lands in the target type
// and must match a generated generic (dangling ones error in user language —
// ui fixtures `at_num_in_type` / `at_group_in_type`).
#[batch_impl((()^2)^Box<@0> where{@1: Clone})]
trait AtNumInType {}

#[batch_impl((()^2)^Box<@0_1>)]
trait AtGroupInType {}

#[test]
fn at_refs_in_target_type() {
    fn check_num<T: AtNumInType>() {}
    check_num::<(u8, u16, Box<u8>)>();
    fn check_group<T: AtGroupInType>() {}
    check_group::<(u8, u16, Box<u16>)>();
}
