//! A per-element `{body}` inside a comma-less bracket group: the group is the slice
//! type, so the body would be emitted as part of the element and reach rustc as
//! `expected ';' or ']'`, with nothing pointing at the attribute. A probe spent
//! compiles on it; the reference documents the list spelling that works.
use batch_impl::batch_impl;

#[batch_impl([u8 { fn k() -> u8 { 1 } }])]
trait Bare {
    fn k() -> u8;
}

#[batch_impl(<T> [Cell<T> { fn k() -> u8 { 2 } }])]
trait Generic {
    fn k() -> u8;
}

fn main() {}
