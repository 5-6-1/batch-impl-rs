//! A bare number or range never became a type. Rendering `impl Tr for 1 {}` /
//! `impl Tr for 0 .. 3 {}` handed rustc a parse error with no `batch-impl:` prefix,
//! and that error suppressed the crate's other diagnostics — so it looked like
//! nothing else was wrong.
use batch_impl::batch_impl;

#[batch_impl(1)]
trait Numbered {
    fn numbered(&self) -> u8;
}

#[batch_impl(0..3)]
trait Ranged {
    fn ranged(&self) -> u8;
}

fn main() {}
