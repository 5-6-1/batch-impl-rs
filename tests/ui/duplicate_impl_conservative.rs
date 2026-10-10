//! General overlap and opaque Rust syntax stay with rustc's coherence check.
use batch_impl::batch_impl;

#[batch_impl(<T: Clone> Vec<T>, <U: Copy> Vec<U>)]
trait DifferentBounds {}

#[batch_impl(#[cfg(all())] [u8, u8])]
trait ConfiguredOverlap {}

// An unrecognized attribute may change the impl. This particular one is a
// no-op, so Rust will still see the duplicate after expanding it.
#[batch_impl(#[batch_impl()] [u16, u16])]
trait OpaqueAttribute {}

macro_rules! target {
    () => {
        u32
    };
}

#[batch_impl(target!(), target!())]
trait OpaqueType {}

macro_rules! length {
    () => {
        2
    };
}

#[batch_impl([u8; length!()], [u8; length!()])]
trait OpaqueConst {}

fn main() {}
