#![allow(dead_code)]
//! Qualified types in specs — `<T as Tr>::Assoc` (a projection head),
//! `Foo<T>::Assoc` and `Foo::<u8>::Assoc` — as a target, as a generic argument
//! and inside a bound. `angle_collect` pairs every `<...>` into a group, so
//! before this the `::`-tail had nowhere to attach and every one of these
//! spellings rendered "unexpected `:` after the type"; the parse-layer lock
//! lives in `src/parse/mod.rs` (`qualified_types_round_trip` /
//! `existing_path_spellings_are_unchanged`), these are the end-to-end ones.
//!
//! A non-projection qualified path with args (`Alias<u8>::Assoc`) is accepted by
//! the parser but is usually E0223 in rustc without `as` (that *semantic* rule
//! belongs to rustc), so the compiled cases below use the projection form; the
//! `Foo<T>::Assoc` and turbofish shapes are covered by the parse tests.

use batch_impl::batch_impl;

trait Tr {
    type Assoc;
}

struct S;
impl Tr for S {
    type Assoc = u8;
}
impl Tr for u8 {
    type Assoc = u16;
}

struct Holder<T>(T);
struct Wrap<T>(T);

// --- a projection as a **generic argument** of the target -------------------

trait M1 {
    fn one(&self) -> u8;
}

#[batch_impl(Vec<<S as Tr>::Assoc>)]
impl M1 for Vec<u8> {
    fn one(&self) -> u8 {
        1
    }
}

#[test]
fn projection_inside_the_target() {
    assert_eq!(Vec::<u8>::new().one(), 1);
}

// --- a projection as the **target**, through the attribute entry ------------

#[batch_impl(M2 <S as Tr>::Assoc { fn two(&self) -> u8 { 2 } })]
trait M2 {
    fn two(&self) -> u8;
}

#[test]
fn projection_as_the_target() {
    assert_eq!(2u8.two(), 2);
}

// --- a nested projection (`<<S as Tr>::Assoc as Tr>::Assoc` = `u16`) --------

trait M3 {
    fn three(&self) -> u8;
}

#[batch_impl(<<S as Tr>::Assoc as Tr>::Assoc)]
impl M3 for u16 {
    fn three(&self) -> u8 {
        3
    }
}

#[test]
fn nested_projection_as_the_target() {
    assert_eq!(3u16.three(), 3);
}

// --- turbofish (`Vec::<u8>`, normalized to `Vec<u8>`) -----------------------

trait M4 {
    fn four(&self) -> u8;
}

#[batch_impl(std::vec::Vec::<u8>)]
impl M4 for Vec<u8> {
    fn four(&self) -> u8 {
        4
    }
}

#[test]
fn turbofish_target() {
    assert_eq!(Vec::<u8>::new().four(), 4);
}

// --- a qualified type as a **shape template** (head structured) ------------
//
// The template `<T as Tr>::Assoc` matches the leaf `<u8 as Tr>::Assoc`:
// head-structured (the head slot `T` binds the leaf head `u8`), tail-verbatim
// (the `as Tr` trait path and the `Assoc` segment compare token-by-token). The
// item's for-type is then slot-substituted — `Holder<T>` → `Holder<u8>`.
trait M6 {
    fn six(&self) -> u8;
}

#[batch_impl(<T as Tr>::Assoc : <u8 as Tr>::Assoc)]
impl M6 for Holder<T> {
    fn six(&self) -> u8 {
        6
    }
}

#[test]
fn qualified_shape_template() {
    assert_eq!(Holder(0u8).six(), 6);
}

// --- a projection inside a **where predicate** -----------------------------

#[batch_impl(<T: Tr> M7<T> Wrap<T> where <T as Tr>::Assoc: Clone { fn seven(&self) -> u8 { 7 } })]
trait M7<T> {
    fn seven(&self) -> u8;
}

#[test]
fn projection_inside_a_where_predicate() {
    assert_eq!(Wrap(0u8).seven(), 7);
}

// --- a projection inside a **bound's** arguments ---------------------------

#[batch_impl(<T: From<<S as Tr>::Assoc>> Wrap<T> { fn five(&self) -> u8 { 5 } })]
trait M5 {
    fn five(&self) -> u8;
}

#[test]
fn projection_inside_a_bound() {
    assert_eq!(Wrap(0u8).five(), 5);
}
