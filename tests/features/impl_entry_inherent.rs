#![allow(dead_code)]
//! The inherent-impl entry: `#[batch_impl(spec)] impl Type { ... }` — the
//! same `template : matrix` spec grammar as the trait-impl entry,
//! with no trait path: `@trait` is banned and the
//! rendered impl has no `for` section.

use batch_impl::batch_impl;

// ---- shape form: template : matrix (leaves mirror the template's shape,
// slots bound per leaf — exactly like the trait-impl entry) ----
struct NumWrap<T> {
    value: T,
}

#[batch_impl(NumWrap<T> : [NumWrap<u8>, NumWrap<u16>])]
impl NumWrap<T> {
    fn into_value(self) -> T {
        self.value
    }
}

#[test]
fn inherent_shape_matrix() {
    assert_eq!(NumWrap::<u8> { value: 3 }.into_value(), 3);
    assert_eq!(NumWrap::<u16> { value: 7 }.into_value(), 7);
}

// ---- a single-leaf source with a new generic declaration (N = 1) ----
#[batch_impl(@Self: <T> Wrap2<T>)]
impl Wrap2<T> {
    fn tag(&self) -> u8 {
        1
    }
}

struct Wrap2<T> {
    _v: T,
}

#[test]
fn inherent_single_leaf_with_generic_declaration() {
    let w = Wrap2 { _v: 1u8 };
    assert_eq!(w.tag(), 1);
}

// ---- where predicates + new-generic-decl compose as usual ----
#[batch_impl(@Self: <'a, T: Clone> Wrap3<T>)]
impl Wrap3<T>
where
    T: Clone,
{
    fn cloned(&self) -> T {
        self.0.clone()
    }
}

struct Wrap3<T>(T);

#[test]
fn inherent_where_and_decl() {
    let w = Wrap3(5u32);
    assert_eq!(w.cloned(), 5);
}
