//! Ext 2 variadic-segment (`ident@..`) integration: template segments bound
//! to tuple elements (name numbering aligned with the leaf position), body
//! repeat blocks (`@(...)..`) with `@ident` name references and `@N` index
//! cursors, and the `@all_fresh` where selector on the generated impls.
//!
//! The `::from` calls below are deliberate: they exercise the slot-name →
//! bound-type rewrite through an identity `From` (clippy-noisy by design).

#![allow(clippy::useless_conversion)]

use batch_impl::batch_impl;

// ------------------------------------------------------------
// 1. alga2-style end-to-end: `()^1..=4` fresh-generic tuples, a variadic
//    segment template, `@all_fresh` where predicates and a `#combine` body
//    repeat block — one spec covers every arity.
// ------------------------------------------------------------
#[batch_impl(
    ()^1..=4 where{@all_fresh: Magma} impl{(A@..,)}
    #combine{( @(@A::combine(&self.@0, &rhs.@0),).. )}
)]
trait Magma {
    fn combine(&self, rhs: &Self) -> Self;
}

impl Magma for u8 {
    fn combine(&self, rhs: &Self) -> Self {
        *self + *rhs
    }
}
impl Magma for u16 {
    fn combine(&self, rhs: &Self) -> Self {
        *self + *rhs
    }
}
impl Magma for u32 {
    fn combine(&self, rhs: &Self) -> Self {
        *self + *rhs
    }
}

#[test]
fn tuple_magma_combine_all_arities() {
    assert_eq!((1u8,).combine(&(10u8,)), (11u8,));
    assert_eq!((1u8, 2u16).combine(&(10u8, 20u16)), (11u8, 22u16));
    assert_eq!((1u8, 2u16, 3u32).combine(&(10u8, 20u16, 30u32)), (11u8, 22u16, 33u32));
    assert_eq!(
        (1u8, 2u16, 3u32, 4u32).combine(&(10u8, 20u16, 30u32, 40u32)),
        (11u8, 22u16, 33u32, 44u32)
    );
}

// ------------------------------------------------------------
// 2. Fixed elements before the segment: the segment starts at leaf index 1,
//    so names are A1/A2 and the index cursor must be `@1`.
// ------------------------------------------------------------
#[batch_impl((u8, u16, u32) impl{(u8, A@..,)} { fn tail(&self) -> (u16, u32) { (@(@A::from(self.@1),)..) } })]
trait ShapeTail {
    fn tail(&self) -> (u16, u32);
}

#[test]
fn offset_start_segment() {
    let t = (1u8, 2u16, 3u32);
    assert_eq!(t.tail(), (2u16, 3u32));
}

// ------------------------------------------------------------
// 3. Nested tuples: two segments in different inner tuples, expanded by two
//    side-by-side blocks with explicit nested paths (`self.0.@0`). Each
//    block's body carries its own trailing-comma separator, so no comma is
//    written between the blocks.
// ------------------------------------------------------------
#[batch_impl(((u8, u16), (u32,)) impl{((A@..,),(B@..,))} { fn flat(&self) -> (u8, u16, u32) { (@(@A::from(self.0.@0),).. @(@B::from(self.1.@0),)..) } })]
trait ShapeFlat {
    fn flat(&self) -> (u8, u16, u32);
}

#[test]
fn nested_segments() {
    let t = ((1u8, 2u16), (3u32,));
    assert_eq!(t.flat(), (1u8, 2u16, 3u32));
}

// ------------------------------------------------------------
// 4. Two same-level segments split the leaf evenly (`(A@.., B@..,)` on an
//    arity-4 leaf → A len 2, B len 2, names A0/A1/B2/B3); one shared cursor
//    round takes the i-th element of both segments.
// ------------------------------------------------------------
#[batch_impl((u8, u16, u32, u32) impl{(A@.., B@..,)} { fn pairs(&self) -> (u64, u64) { (@(@A::from(self.@0) as u64 + @B::from(self.@2) as u64,)..) } })]
trait ShapePairs {
    fn pairs(&self) -> (u64, u64);
}

#[test]
fn multi_segment_parallel_rounds() {
    let t = (1u8, 2u16, 3u32, 4u32);
    assert_eq!(t.pairs(), (4, 6));
}

// ------------------------------------------------------------
// 5. A single-element segment (`(A@..,)` on an arity-1 leaf) and a segment
//    name used directly (not via `@ident`) in the body.
// ------------------------------------------------------------
#[batch_impl((u8,) impl{(A@..,)} { fn get(&self) -> A0 { self.0 } })]
trait ShapeOne {
    fn get(&self) -> u8;
}

#[test]
fn single_element_segment_direct_name() {
    assert_eq!((7u8,).get(), 7);
}

// ------------------------------------------------------------
// 6. alga2 tuple Module: the scalar of every component from the second one
//    on must equal the first component's scalar — `@1..` open-range where
//    predicates with a `Scalar = @0::Scalar` value reference inside the
//    angle group (arity 1 contributes no such predicate).
// ------------------------------------------------------------
#[batch_impl(
    Module<(), ()> ()^1..=4 where{
        @all_fresh: Module<(), (), Scalar: Copy>,
        @1..: Module<(), (), Scalar = @0::Scalar>,
    } impl{(A@..,)}
    #Scalar{A0::Scalar}
    #scale{( @(@A::scale(&self.@0, s),).. )}
)]
trait Module<Add, Mul> {
    type Scalar;
    fn scale(&self, s: Self::Scalar) -> Self;
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct S<T>(T);

impl<T: Copy + std::ops::Mul<Output = T>> Module<(), ()> for S<T> {
    type Scalar = T;
    fn scale(&self, s: T) -> Self {
        S(self.0 * s)
    }
}

#[test]
fn tuple_module_shared_scalar() {
    assert_eq!((S(2u8),).scale(4u8), (S(8u8),));
    assert_eq!((S(2u8), S(3u8)).scale(4u8), (S(8u8), S(12u8)));
    assert_eq!((S(2u8), S(3u8), S(4u8)).scale(4u8), (S(8u8), S(12u8), S(16u8)));
}

// ------------------------------------------------------------
// 7. Cursor-only blocks: `@(self.@0,)..` with no `@ident` — the length
//    comes from the template's unique segment (implicit) or from a declared
//    driver (`@A(self.@0,)..`).
// ------------------------------------------------------------
#[batch_impl((u8, u16, u32) impl{(A@..,)} { fn elems(&self) -> (u8, u16, u32) { (@(self.@0,)..) } })]
trait ShapeElems {
    fn elems(&self) -> (u8, u16, u32);
}

#[batch_impl((u8, u16, u32) impl{(A@..,)} { fn elems2(&self) -> (u8, u16, u32) { (@A(self.@0,)..) } })]
trait ShapeElemsDeclared {
    fn elems2(&self) -> (u8, u16, u32);
}

#[test]
fn cursor_only_blocks() {
    let t = (1u8, 2u16, 3u32);
    assert_eq!(t.elems(), (1u8, 2u16, 3u32));
    assert_eq!(t.elems2(), (1u8, 2u16, 3u32));
}
