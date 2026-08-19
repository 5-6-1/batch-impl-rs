//! Ext 1 (0.8.0) ItemImpl trait/where tests: `@trait` in generic-decl
//! bounds and where predicates, the impl's own generics/where preservation,
//! and shape-template slots rewritten in where predicates.
//! (split from the former single-file `tests/ext1_impl.rs`)

use batch_impl::batch_impl;

// ------------------------------------------------------------
// 4. `@trait` in a new-generic-decl bound → the impl's trait path
// ------------------------------------------------------------
#[batch_impl(<T: @trait> Box<T>)]
impl Mk4 for Box<T> {
    fn tag(&self) -> u32 {
        4
    }
}

impl Mk4 for u8 {
    fn tag(&self) -> u32 {
        4
    }
}

trait Mk4 {
    fn tag(&self) -> u32;
}

#[test]
fn ext1_at_trait_bound() {
    assert_eq!(<Box<u8> as Mk4>::tag(&Box::new(1)), 4);
}

// ------------------------------------------------------------
// 5. `@trait` in a bare where predicate (trait path substitution)
// ------------------------------------------------------------
#[batch_impl(<T> Box<T> where T: @trait)]
impl Mk5 for Box<T> {
    fn tag(&self) -> u32 {
        5
    }
}

impl Mk5 for u8 {
    fn tag(&self) -> u32 {
        5
    }
}

trait Mk5 {
    fn tag(&self) -> u32;
}

#[test]
fn ext1_at_trait_where() {
    assert_eq!(<Box<u8> as Mk5>::tag(&Box::new(1)), 5);
}

// ------------------------------------------------------------
// 6. The impl's own generics / where clause are preserved
// ------------------------------------------------------------
#[batch_impl(Box<T> where T: Clone)]
impl<T: Clone> Mk6 for Box<T> {
    fn n(&self) -> usize {
        6
    }
}

trait Mk6 {
    fn n(&self) -> usize;
}

#[test]
fn ext1_impl_own_generics_where() {
    assert_eq!(<Box<u8> as Mk6>::n(&Box::new(1)), 6);
}

// ------------------------------------------------------------
// 8. Shape-template slots in the where predicates are rewritten too
// ------------------------------------------------------------
#[batch_impl(A<B> : Vec.u8 where A<B>: Clone)]
impl Mk8 for A<B> {
    fn n(&self) -> usize {
        self.len()
    }
}

trait Mk8 {
    fn n(&self) -> usize;
}

#[test]
fn ext1_where_slot_rewrite() {
    let v: Vec<u8> = vec![1, 2, 3];
    assert_eq!(v.n(), 3);
}
