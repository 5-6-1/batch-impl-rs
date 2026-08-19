//! Ext 1 (0.8.0) basic ItemImpl entry tests: single-level matrix, the
//! direct form, `;`-separated multi-spec, and `unsafe impl` preservation.
//! (split from the former single-file `tests/ext1_impl.rs`)

use batch_impl::batch_impl;
use std::rc::Rc;

// ------------------------------------------------------------
// 1. Single-level matrix: `A<B> : [Box, Rc].[u8, u16]` → 4 impls
// ------------------------------------------------------------
#[batch_impl(A<B> : [Box, Rc].[u8, u16])]
impl Mk1 for A<B> {
    fn mk() -> A<B> {
        A::new(B::default())
    }
}

trait Mk1 {
    fn mk() -> Self;
}

#[test]
fn ext1_basic_matrix() {
    let b: Box<u8> = <Box<u8> as Mk1>::mk();
    assert_eq!(*b, 0);
    let r: Rc<u16> = <Rc<u16> as Mk1>::mk();
    assert_eq!(*r, 0);
    let _: Box<u16> = <Box<u16> as Mk1>::mk();
    let _: Rc<u8> = <Rc<u8> as Mk1>::mk();
}

// ------------------------------------------------------------
// 2. Direct form: `new-generic-decl? for-type` (no matrix, N = 1)
// ------------------------------------------------------------
#[batch_impl(<T> Box<T>)]
impl Mk2 for Box<T> {
    fn tag(&self) -> u32 {
        2
    }
}

trait Mk2 {
    fn tag(&self) -> u32;
}

#[test]
fn ext1_direct_form() {
    assert_eq!(<Box<i32> as Mk2>::tag(&Box::new(5)), 2);
    assert_eq!(<Box<u16> as Mk2>::tag(&Box::new(5)), 2);
}

// ------------------------------------------------------------
// 3. Multiple specs (`;`-separated): `W:u8; W:u16`
// ------------------------------------------------------------
#[batch_impl(W:u8; W:u16)]
impl Mk3 for W {
    fn bits() -> u32 {
        W::BITS
    }
}

trait Mk3 {
    fn bits() -> u32;
}

#[test]
fn ext1_multi_spec_semicolon() {
    assert_eq!(<u8 as Mk3>::bits(), 8);
    assert_eq!(<u16 as Mk3>::bits(), 16);
}

// ------------------------------------------------------------
// 7. `unsafe impl` preserved
// ------------------------------------------------------------
#[batch_impl(U:u8; U:u16)]
unsafe impl Mk7 for U {
    const TAG: u32 = U::BITS;
}

/// # Safety
///
/// Marker trait for the demo only; no real unsafe semantics.
unsafe trait Mk7 {
    const TAG: u32;
}

#[test]
fn ext1_unsafe_impl() {
    assert_eq!(<u8 as Mk7>::TAG, 8);
    assert_eq!(<u16 as Mk7>::TAG, 16);
}
