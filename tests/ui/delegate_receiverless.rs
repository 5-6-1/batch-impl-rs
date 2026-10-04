//! A `#delegate` body reaching a receiver-less method through `self`.
//!
//! Measured before the fix: this generated `(self).associated()` and died with a bare E0424
//! (`this function doesn't have a 'self' parameter`) that never mentioned `#delegate`, while
//! the macro knew the method had no receiver all along.
use batch_impl::batch_impl;

#[batch_impl(u8 #delegate(@all_static_methods){self})]
pub trait Static {
    fn associated() -> i8;
}

fn main() {}
