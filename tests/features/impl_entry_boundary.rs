//! The impl entry (0.8.0) boundary-case tests: empty matrix source,
//! direct form without new-generic-decl, three-plus `;`-separated specs,
//! matrix chains with modifiers, and the zero-slot template.
//! (new test module; split-style organization per the features/ layout)

use batch_impl::batch_impl;
use std::rc::Rc;

// ------------------------------------------------------------
// 1. Empty matrix source (nothing after `:`): N = 1, the shape itself —
//    a template without slots (the for-Type is emitted verbatim)
// ------------------------------------------------------------
#[batch_impl(Vec<u8> : )]
impl BndMk1 for Vec<u8> {
    fn n1(&self) -> usize {
        self.len()
    }
}

trait BndMk1 {
    fn n1(&self) -> usize;
}

#[test]
fn impl_entry_empty_matrix_source() {
    let b = Box::new(vec![1u8, 2, 3]);
    assert_eq!(b.n1(), 3);
}

// ------------------------------------------------------------
// 2. Direct form without a new-generic-decl: `Vec<u8>` alone (N = 1)
// ------------------------------------------------------------
#[batch_impl(Vec<u8>)]
impl BndMk2 for Vec<u8> {
    fn n2(&self) -> usize {
        self.len()
    }
}

trait BndMk2 {
    fn n2(&self) -> usize;
}

#[test]
fn impl_entry_direct_no_generics() {
    let v = vec![1u8, 2];
    assert_eq!(v.n2(), 2);
}

// ------------------------------------------------------------
// 3. Three-plus `;`-separated specs sharing the impl
// ------------------------------------------------------------
#[batch_impl(W:u8; W:u16; W:u32)]
impl BndMk3 for W {
    fn bits() -> u32 {
        W::BITS
    }
}

trait BndMk3 {
    fn bits() -> u32;
}

#[test]
fn impl_entry_three_specs() {
    assert_eq!(<u8 as BndMk3>::bits(), 8);
    assert_eq!(<u16 as BndMk3>::bits(), 16);
    assert_eq!(<u32 as BndMk3>::bits(), 32);
}

// ------------------------------------------------------------
// 4. Matrix chain with a reference modifier: `&.[Box, Rc].u8` →
//    `&Box<u8>` / `&Rc<u8>`; the template mirrors the reference (`&A<B>`)
// ------------------------------------------------------------
#[batch_impl(&A<B> : &.[Box, Rc].u8)]
impl BndMk4 for &A<B> {
    fn get(&self) -> B {
        ***self
    }
}

trait BndMk4 {
    fn get(&self) -> u8;
}

#[test]
fn impl_entry_matrix_with_modifiers() {
    let b = Box::new(5u8);
    let rb = &b;
    assert_eq!(rb.get(), 5);
    let r = Rc::new(7u8);
    let rr = &r;
    assert_eq!(rr.get(), 7);
}

// ------------------------------------------------------------
// 5. Zero-slot template (all literals, no binding): `Vec<u8> : Vec.u8`
//    — the leaf equals the template ident-for-ident, no mapping is built
// ------------------------------------------------------------
#[batch_impl(Vec<u8> : Vec.u8)]
impl BndMk5 for Vec<u8> {
    fn n5(&self) -> usize {
        self.len()
    }
}

trait BndMk5 {
    fn n5(&self) -> usize;
}

#[test]
fn impl_entry_zero_slot_template() {
    let v = vec![1u8];
    assert_eq!(v.n5(), 1);
}

// ------------------------------------------------------------
// 6. Empty spec list: the attribute derives nothing, so it is a **no-op** and
//    the original block is emitted unchanged. The bare form, the empty-parens
//    form and a separators-only list all count as empty. (Before the guard the
//    impl was withheld and nothing replaced it: the file still compiled, so
//    only the `tag` calls below can tell the difference.)
// ------------------------------------------------------------
#[batch_impl]
impl BndMk6 for u8 {
    fn tag(&self) -> u32 {
        7
    }
}

#[batch_impl()]
impl BndMk6 for u16 {
    fn tag(&self) -> u32 {
        9
    }
}

#[batch_impl(;)]
impl BndMk6 for u32 {
    fn tag(&self) -> u32 {
        11
    }
}

trait BndMk6 {
    fn tag(&self) -> u32;
}

#[test]
fn impl_entry_empty_spec_list_is_a_no_op() {
    assert_eq!(0u8.tag(), 7);
    assert_eq!(0u16.tag(), 9);
    assert_eq!(0u32.tag(), 11);
}
