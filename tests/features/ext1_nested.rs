//! Ext 1 (0.8.0) nested/multi-dimensional ItemImpl tests: nested matrix
//! sources (inner array distribution), multi-level generic templates, and
//! slot binding to composite subtrees.
//! (new test module; split-style organization per the features/ layout)

use batch_impl::batch_impl;
use std::rc::Rc;

// ------------------------------------------------------------
// 1. Nested matrix source: `[u8, [u16, u32]]` distributes to u8/u16/u32
//    (inner array recursion) → 6 leaves
// ------------------------------------------------------------
#[batch_impl(A<B> : [Box, Rc].[u8, [u16, u32]])]
impl NstMk1 for A<B> {
    fn mk() -> A<B> {
        A::new(B::default())
    }
}

trait NstMk1 {
    fn mk() -> Self;
}

#[test]
fn ext1_nested_matrix_distribution() {
    let _: Box<u8> = <Box<u8> as NstMk1>::mk();
    let _: Box<u16> = <Box<u16> as NstMk1>::mk();
    let _: Box<u32> = <Box<u32> as NstMk1>::mk();
    let _: Rc<u8> = <Rc<u8> as NstMk1>::mk();
    let _: Rc<u16> = <Rc<u16> as NstMk1>::mk();
    let _: Rc<u32> = <Rc<u32> as NstMk1>::mk();
}

// ------------------------------------------------------------
// 2. Multi-level generic template: `A<B<C>>` matches `Box<Vec<u8>>`
//    (A := Box, B := Vec, C := u8) — three nesting levels of slots
// ------------------------------------------------------------
#[batch_impl(A<B<C>> : Box.Vec.u8)]
impl NstMk2 for A<B<C>> {
    fn head(&self) -> C {
        self[0]
    }
}

trait NstMk2 {
    fn head(&self) -> u8;
}

#[test]
fn ext1_multi_level_template() {
    let v: Box<Vec<u8>> = Box::new(vec![7u8]);
    assert_eq!(v.head(), 7);
}

// ------------------------------------------------------------
// 3. Slot bound to a composite subtree: `A<B> : Vec.[u8, String]` —
//    B is bound to the whole leaf arg (u8 / String)
// ------------------------------------------------------------
#[batch_impl(A<B> : Vec.[u8, String])]
impl NstMk3 for A<B> {
    fn mk() -> A<B> {
        A::new()
    }
}

trait NstMk3 {
    fn mk() -> Self;
}

#[test]
fn ext1_slot_composite_subtree() {
    let v: Vec<u8> = <Vec<u8> as NstMk3>::mk();
    assert!(v.is_empty());
    let s: Vec<String> = <Vec<String> as NstMk3>::mk();
    assert!(s.is_empty());
}

// ------------------------------------------------------------
// 4. Multi-dimensional matrix (two `-` layers): `Pair2-[u8, u16]-[i8, i16]`
//    → Pair2<u8, i8> ... (left-assoc `-` accumulates the two arg lists; the
//    template keeps two slots; the trait signature stays slot-free)
// ------------------------------------------------------------
struct Pair2<A, B>(A, B);
#[batch_impl(P<Q, R> : Pair2-[u8, u16]-[i8, i16])]
impl NstMk4 for P<Q, R> {
    fn mk() -> P<Q, R> {
        P(Q::default(), R::default())
    }
}

trait NstMk4 {
    fn mk() -> Self;
}

#[test]
fn ext1_multi_dimensional_matrix() {
    let p = <Pair2<u8, i8> as NstMk4>::mk();
    assert_eq!(p.0, 0);
    assert_eq!(p.1, 0);
    let q = <Pair2<u16, i16> as NstMk4>::mk();
    assert_eq!(q.0, 0);
    assert_eq!(q.1, 0);
}

// ------------------------------------------------------------
// 5. Matrix + where combined: slots in the where clause, a container
//    matrix with `self.len()` bodies
// ------------------------------------------------------------
#[batch_impl(A<B> : Vec.[u8, u16] where A<B>: Sized)]
impl NstMk5 for A<B> {
    fn n(&self) -> usize {
        self.len()
    }
}

trait NstMk5 {
    fn n(&self) -> usize;
}

#[test]
fn ext1_matrix_where_combined() {
    let v: Vec<u8> = vec![1, 2];
    assert_eq!(v.n(), 2);
    let w: Vec<u16> = vec![1u16];
    assert_eq!(w.n(), 1);
}
