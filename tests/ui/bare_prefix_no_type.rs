//! A bare type prefix: `&` with nothing to modify.
//!
//! The leaked-carrier check named the pointer prefixes and `self`, so `&`, `&mut` and
//! `unsafe` fell through and the target rendered as `impl Tr for & {}` - not Rust, and
//! rustc's parse error never says what went wrong.
use batch_impl::batch_impl;

#[batch_impl(&)]
trait BareRef {}

#[batch_impl(unsafe)]
trait BareUnsafe {}

fn main() {}
