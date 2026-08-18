//! Ext 2 (0.8.0) basic `impl{...}` tests: single-slot binding, equal-base
//! literals, different-base slots, matrix distribution, and any-order
//! attachments.
//! (split from the former single-file `tests/ext2_impl.rs`)

use batch_impl::batch_impl;
use std::rc::Rc;

// ------------------------------------------------------------
// 1. `impl{T}` + i32 → T := i32 (bare ident template binds the whole leaf)
// ------------------------------------------------------------
#[batch_impl(i32 impl{T} { fn bits(&self) -> u32 { T::BITS } })]
trait ImplT {
    fn bits(&self) -> u32;
}

#[test]
fn impl_single_slot() {
    assert_eq!(0i32.bits(), 32);
}

// ------------------------------------------------------------
// 2. `impl{Rc<T>}` + Rc<i32> → T := i32 (Rc is a literal: equal)
// ------------------------------------------------------------
#[batch_impl(Rc<i32> impl{Rc<T>} { fn bits(&self) -> u32 { T::BITS } })]
trait ImplRcT {
    fn bits(&self) -> u32;
}

#[test]
fn impl_equal_base_literal() {
    let r = Rc::new(0i32);
    assert_eq!(r.bits(), 32);
}

// ------------------------------------------------------------
// 3. `impl{Rc<T>}` + Box<i32> → Rc := Box, T := i32 (different base → slot)
// ------------------------------------------------------------
#[batch_impl(Box<i32> impl{Rc<T>} { fn mk(x: i32) -> Rc<T> { Rc::new(x) } })]
trait ImplRcSlot {
    fn mk(x: i32) -> Self;
}

#[test]
fn impl_different_base_slot() {
    let b: Box<i32> = <Box<i32> as ImplRcSlot>::mk(7);
    assert_eq!(*b, 7);
}

// ------------------------------------------------------------
// 4. Matrix leaves × one template: `[Box, Rc]^u32 impl{W<T>}` — the base
//    slot W binds each leaf's base (Box / Rc), the arg slot T binds u32
// ------------------------------------------------------------
#[batch_impl([Box, Rc]^u32 impl{W<T>} { fn mk(x: u32) -> W<T> { W::new(x) } })]
trait ImplMatrix {
    fn mk(x: u32) -> Self;
}

#[test]
fn impl_matrix_distribution() {
    assert_eq!(*<Box<u32> as ImplMatrix>::mk(5), 5);
    assert_eq!(*<Rc<u32> as ImplMatrix>::mk(6), 6);
}

// ------------------------------------------------------------
// 5. Attachment in any order: `T {body} impl{X}` (impl after the body)
// ------------------------------------------------------------
#[batch_impl(i32 { fn b8(&self) -> u32 { 8 } } impl{X})]
trait ImplOrderAny {
    fn b8(&self) -> u32;
}

#[test]
fn impl_attachment_any_order() {
    assert_eq!(0i32.b8(), 8);
}
