//! A stray `#` used to drop the whole spec silently: the trait was emitted, zero
//! impls were generated, and no diagnostic pointed at the attribute — the user only
//! saw rustc's `no method named ...` at some call site. Both spellings are now
//! targeted errors.
use batch_impl::batch_impl;

#[batch_impl(#)]
trait V {
    fn v(&self) -> u8;
}

#[batch_impl(#{0})]
trait W {
    fn w(&self) -> u8;
}

fn main() {}
