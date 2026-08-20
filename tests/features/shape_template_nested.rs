//! The `impl{...}` shape templates (0.8.0) nested/container-template tests: multi-level
//! generic templates, reference/pointer templates, tuple templates, and
//! array/slice templates.
//! (new test module; split-style organization per the features/ layout)

use batch_impl::batch_impl;
use std::rc::Rc;

// ------------------------------------------------------------
// 1. Multi-level template: `impl{A<B<C>>}` matches `Vec<Vec<u8>>`
//    (A := Vec, B := Vec, C := u8)
// ------------------------------------------------------------
#[batch_impl(Vec<Vec<u8>> impl{A<B<C>>} { fn n(&self) -> usize { self.len() } })]
trait NestT1 {
    fn n(&self) -> usize;
}

#[test]
fn impl_nested_template() {
    let v = vec![vec![1u8], vec![2u8]];
    assert_eq!(v.n(), 2);
}

// ------------------------------------------------------------
// 2. Reference template: `impl{&A}` matches `&u32` (A := u32)
// ------------------------------------------------------------
#[batch_impl(&u32 impl{&A} { fn val(&self) -> A { **self } })]
trait NestT2 {
    fn val(&self) -> u32;
}

#[test]
fn impl_reference_template() {
    let x = 42u32;
    assert_eq!((&x).val(), 42);
}

// ------------------------------------------------------------
// 3. `&mut` template
// ------------------------------------------------------------
#[batch_impl(&mut u16 impl{&mut A} { fn val(&self) -> A { **self } })]
trait NestT3 {
    fn val(&self) -> u16;
}

#[test]
fn impl_refmut_template() {
    let mut x = 7u16;
    let r = &mut x;
    assert_eq!(r.val(), 7);
}

// ------------------------------------------------------------
// 4. Raw-pointer template: `impl{*const A}` matches `*const u8`
// ------------------------------------------------------------
#[batch_impl(*const u8 impl{*const A} { fn deref_unsafe(&self) -> A { unsafe { **self } } })]
trait NestT4 {
    fn deref_unsafe(&self) -> u8;
}

#[test]
fn impl_ptr_template() {
    let x = 9u8;
    let p = &x as *const u8;
    assert_eq!(p.deref_unsafe(), 9);
}

// ------------------------------------------------------------
// 5. Tuple template: `impl{(A, B)}` matches `(u8, u16)` (A := u8, B := u16)
// ------------------------------------------------------------
#[batch_impl((u8, u16) impl{(A, B)} { fn sum(&self) -> u32 { self.0 as u32 + self.1 as u32 } })]
trait NestT5 {
    fn sum(&self) -> u32;
}

#[test]
fn impl_tuple_template() {
    let t = (1u8, 2u16);
    assert_eq!(t.sum(), 3);
}

// ------------------------------------------------------------
// 6. Fixed-array template: `impl{[A; 2]}` matches `[u8; 2]`
//    (the length literal compares verbatim; A := u8)
// ------------------------------------------------------------
#[batch_impl([u8; 2] impl{[A; 2]} { fn n(&self) -> usize { A::BITS as usize * self.len() } })]
trait NestT6 {
    fn n(&self) -> usize;
}

#[test]
fn impl_array_template() {
    let a = [1u8, 2];
    assert_eq!(a.n(), 16); // u8::BITS (8) × 2
}

// ------------------------------------------------------------
// 7. Slice template: `impl{[A]}` matches `[u8]`
// ------------------------------------------------------------
#[batch_impl([u8] impl{[A]} { fn n(&self) -> usize { self.len() } })]
trait NestT7 {
    fn n(&self) -> usize;
}

#[test]
fn impl_slice_template() {
    let s: &[u8] = &[1, 2, 3];
    assert_eq!(s.n(), 3);
}

// ------------------------------------------------------------
// 8. Container template with an equal base: `impl{Rc<A>}` matches `Rc<i32>`
// ------------------------------------------------------------
#[batch_impl(Rc<i32> impl{Rc<A>} { fn val(&self) -> A { **self } })]
trait NestT8 {
    fn val(&self) -> i32;
}

#[test]
fn impl_container_equal_base() {
    let r = Rc::new(5i32);
    assert_eq!(r.val(), 5);
}
