//! `@N..` / `@N..M` range references in where predicates: the tail after
//! the range is copied per fresh (`@0..::Out: Clone` → `P0::Out: Clone,
//! P1::Out: Clone`), open ranges (`@1..`) skip the first fresh, closed
//! ranges (`@0..=1`) cover exactly a run. The token-level re-opening in
//! type positions (`Wrapper<@0..>`) is covered by `codegen::range_refs`
//! unit tests; these end-to-end tests drive the where-predicate path
//! through the real macro.

use batch_impl::batch_impl;

// ============================================================
// 1. `@0..` open range: every fresh gets the predicate tail.
//    The fresh list comes from the target's `*().3` generator splat.
// ============================================================
struct Triple<T, U, V>(T, U, V);

#[batch_impl(Triple<*().3> where{@0..: Clone} { fn m(&self) {} })]
#[allow(dead_code)]
trait RangeWhereAll {
    fn m(&self);
}

#[test]
fn range_where_all_fresh() {
    fn check<T: RangeWhereAll>(_: &T) {}
    check(&Triple(0u8, 1u16, 2u32));
}

// ============================================================
// 2. `@1..` open range from index 1: the first fresh is unconstrained.
// ============================================================
#[batch_impl(Triple<*().3> where{@1..: Copy} { fn m(&self) {} })]
#[allow(dead_code)]
trait RangeWhereTail {
    fn m(&self);
}

#[test]
fn range_where_tail() {
    fn check<T: RangeWhereTail>(_: &T) {}
    check(&Triple(0u8, 1u16, 2u32));
}

// ============================================================
// 3. `@0..=1` closed range: exactly the first two freshes.
// ============================================================
#[batch_impl(Triple<*().3> where{@0..=1: Copy} { fn m(&self) {} })]
#[allow(dead_code)]
trait RangeWhereClosed {
    fn m(&self);
}

#[test]
fn range_where_closed() {
    fn check<T: RangeWhereClosed>(_: &T) {}
    check(&Triple(0u8, 1u16, 2u32));
}

// ============================================================
// 4. Range with an associated-type path tail: `@0..::Out: Clone` — the
//    `::Out` part is copied per fresh (`P0::Out: Clone, P1::Out: Clone`).
// ============================================================
#[allow(dead_code)]
trait HasOut {
    type Out;
}
struct Wrap2<A, B>(A, B);

#[batch_impl(Wrap2<*().2> where{@0..: HasOut, @0..::Out: Clone} { fn m(&self) {} })]
#[allow(dead_code)]
trait RangeAssocPath {
    fn m(&self);
}

// The `#[batch_impl]` declaration above is itself the test: it must expand
// to `impl ... where P0: HasOut, P0::Out: Clone, P1: HasOut, P1::Out: Clone`
// (the `::Out` tail copied per fresh) without a compile error. No instance
// is constructed — the generated where bounds are the assertion.

// ============================================================
// 5. Two ranges in one where clause: an open `@0..` and a closed `@0..=1`
//    coexist (each expands independently).
// ============================================================
#[batch_impl(Wrap2<*().2> where{@0..: Clone, @0..=1: Copy} { fn m(&self) {} })]
#[allow(dead_code)]
trait RangeWhereCombined {
    fn m(&self);
}

#[test]
fn range_where_combined() {
    fn check<T: RangeWhereCombined>(_: &T) {}
    check(&Wrap2(0u8, 1u16));
}

// ============================================================
// 6. `@all_fresh` remains equivalent to `@0..`.
// ============================================================
#[batch_impl(Wrap2<*().2> where{@all_fresh: Clone} { fn m(&self) {} })]
#[allow(dead_code)]
trait RangeAllFreshEq {
    fn m(&self);
}

#[test]
fn range_all_fresh_equivalence() {
    fn check<T: RangeAllFreshEq>(_: &T) {}
    check(&Wrap2(0u8, 1u16));
}

// ============================================================
// 7. `@0..` in the impl-generic **declaration** position: `<@0..>` declares
//    every fresh the range covers as an impl param. The fresh list comes
//    from the trait-arg generator (`GenConv<*().2>`); the declaration and
//    the where predicate reference the same batch.
// ============================================================
struct DeclTarget;

#[batch_impl(<@0..> GenConvDecl<*().2> DeclTarget where @0..: Clone { fn m(&self) {} })]
#[allow(dead_code)]
trait GenConvDecl<T, U> {
    fn m(&self);
}

#[test]
fn range_decl_position() {
    fn check<T: GenConvDecl<u8, u16>>(_: &T) {}
    check(&DeclTarget);
}

// ============================================================
// 8. Grouped ranges `@L_N..` / `@L_N..M`: a range **within** one generator
//    group (stable across array dispatch, like `@g_i`). Two generators
//    (`<*().2>` → group 0, `<*().3>` → group 1); `@1_0..: Clone` constrains
//    only group 1's three fresh.
// ============================================================
struct MultiTarget;

#[batch_impl(
    <@0..> <@1..> PairGen<*().2, *().3> MultiTarget where{@1_0..: Clone}
    { fn m(&self) {} }
)]
#[allow(dead_code)]
trait PairGen<A, B, C, D, E> {
    fn m(&self);
}

#[test]
fn grouped_range_where() {
    fn check<T: PairGen<u8, u16, u32, u64, u128>>(_: &T) {}
    check(&MultiTarget);
}

// ============================================================
// 9. Grouped range in the declaration position: `<@1_1..>` declares only
//    group 1 from position 1 onward (the group tail).
// ============================================================
#[batch_impl(
    <@0..> <@1_1..> PairGenDecl<*().2, *().3> MultiTarget
    { fn m(&self) {} }
)]
#[allow(dead_code)]
trait PairGenDecl<A, B, C, D, E> {
    fn m(&self);
}

#[test]
fn grouped_range_decl() {
    fn check<T: PairGenDecl<u8, u16, u32, u64, u128>>(_: &T) {}
    check(&MultiTarget);
}

// ============================================================
// 10. Exclusive `@N..M` in a **type position** (target type) normalizes to
//     the inclusive protocol, matching the where-predicate path:
//     `@0..2` covers P0, P1 (not P2). `GenConvX<*().3>` hoists 3 freshs;
//     the 2-param target compiles only if the exclusive range excludes the
//     third. Regression guard: the parse-layer range folding used to keep
//     the raw end (inclusive) here while the where path excluded it — one
//     spelling, two semantics.
// ============================================================
struct Wrap2Ty<A, B>(A, B);

#[batch_impl(GenConvX<*().3> Wrap2Ty<@0..2> { fn m(&self) {} })]
#[allow(dead_code)]
trait GenConvX<A, B, C> {
    fn m(&self);
}

#[test]
fn exclusive_range_in_type_position_is_exclusive() {
    fn check<T: GenConvX<u8, u16, u32>>(_: &T) {}
    check(&Wrap2Ty(0u8, 1u16));
}
