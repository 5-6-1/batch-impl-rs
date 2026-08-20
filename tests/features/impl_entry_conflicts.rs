//! The impl entry (0.8.0) conflict/overlap tests: "looks like a conflict
//! but is legal" cases — fully-literal (zero-binding) templates, slot names
//! colliding with real type names in the body (rewritten), and mixed slots
//! in expressions. Compile-failing conflicts live in `tests/ui/implentry_*`.
//! (new test module; split-style organization per the features/ layout)

use batch_impl::batch_impl;
use std::rc::Rc;

// ------------------------------------------------------------
// 1. Fully-literal template (`Vec<u8> : Vec.u8`): every ident is equal at
//    its position → no binding at all; the body's `Vec` stays untouched
// ------------------------------------------------------------
#[batch_impl(Vec<u8> : Vec.u8)]
impl CfMk1 for Vec<u8> {
    fn n(&self) -> usize {
        self.len()
    }
}

trait CfMk1 {
    fn n(&self) -> usize;
}

#[test]
fn impl_entry_literal_template_no_binding() {
    let v: Vec<u8> = vec![1, 2, 3];
    assert_eq!(v.n(), 3);
}

// ------------------------------------------------------------
// 2. Slot name collides with a real type name in the body: the body ident
//    is rewritten (the slot semantics win — "same name, same entity").
//    `Wrapper` is a slot bound to Box/Rc; `Wrapper::new` → `Box::new`
// ------------------------------------------------------------
#[batch_impl(Wrapper<T> : [Box, Rc].u8)]
impl CfMk2 for Wrapper<T> {
    fn mk() -> Wrapper<T> {
        Wrapper::new(T::default())
    }
}

trait CfMk2 {
    fn mk() -> Self;
}

#[test]
fn impl_entry_slot_name_overrides_body() {
    let b: Box<u8> = <Box<u8> as CfMk2>::mk();
    assert_eq!(*b, 0);
    let r: Rc<u8> = <Rc<u8> as CfMk2>::mk();
    assert_eq!(*r, 0);
}

// ------------------------------------------------------------
// 3. Mixed slots in one expression: `A::new(B::default())` — the base slot
//    in a path, the arg slot as an argument
// ------------------------------------------------------------
#[batch_impl(A<B> : [Box, Rc].[u8, u16])]
impl CfMk3 for A<B> {
    fn mk() -> A<B> {
        A::new(B::default())
    }
}

trait CfMk3 {
    fn mk() -> Self;
}

#[test]
fn impl_entry_mixed_slots_expression() {
    assert_eq!(*<Box<u8> as CfMk3>::mk(), 0);
    assert_eq!(*<Rc<u16> as CfMk3>::mk(), 0);
}

// ------------------------------------------------------------
// 4. Slot in a where predicate on the left side, and the same slot as an
//    associated-type projection bound
// ------------------------------------------------------------
trait HasAssoc2 {
    type Assoc;
}
impl HasAssoc2 for u8 {
    type Assoc = u8;
}
impl HasAssoc2 for u16 {
    type Assoc = u16;
}

#[batch_impl(A<B> : Vec.[u8, u16] where A<B>: Sized, B: HasAssoc2)]
impl CfMk4 for A<B> {
    fn a(&self) -> usize {
        self.len()
    }
}

trait CfMk4 {
    fn a(&self) -> usize;
}

#[test]
fn impl_entry_where_slot_left_and_projection() {
    let v: Vec<u8> = vec![1u8, 2];
    assert_eq!(v.a(), 2);
    let w: Vec<u16> = vec![1u16, 2, 3];
    assert_eq!(w.a(), 3);
}
