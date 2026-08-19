//! dsl.rs generic-parameter family tests: `@all_type_params` /
//! `@all_const_params` / `@all_lifetimes` declarations, and the
//! nested-generator dedup in `(T,).N`.
//! (split from the former single-file `tests/dsl.rs`)

use batch_impl::batch_impl;

#[batch_impl(@all_type_params GenT<T> Vec<T> { fn head(&self) -> T { self[0].clone() } })]
trait GenT<T: Clone> {
    fn head(&self) -> T;
}

#[batch_impl(@all_lifetimes @all_type_params GenB<'a, T> &'a T { fn get(&self) -> &'a T { self } })]
trait GenB<'a, T: Clone> {
    fn get(&self) -> &'a T;
}

#[batch_impl(@all_const_params GenC<N> [u8; N] { fn n(&self) -> usize { N } })]
trait GenC<const N: usize> {
    fn n(&self) -> usize;
}

#[test]
fn generic_param_families() {
    // type params: declaration auto-copied, bounds via same-name inheritance
    let v = vec![1u32, 2];
    assert_eq!(v.head(), 1u32);
    // lifetime + type combination (consecutive declarations keep lifetimes first)
    let b = &5u8;
    assert_eq!(b.get(), &5u8);
    // const params: full `const N: usize` declaration
    let a: [u8; 3] = [1, 2, 3];
    assert_eq!(a.n(), 3);
}

#[batch_impl((().3,).3)]
trait NestedGenT {}

#[test]
fn nested_generator_in_tuple_pow() {
    // (T,).3 clones the generator's fresh declarations; hoisting must
    // dedupe them so the impl has one shared generic trio.
    fn assert_trait<T: NestedGenT>() {}
    assert_trait::<((u8, u16, u32), (u8, u16, u32), (u8, u16, u32))>();
}
