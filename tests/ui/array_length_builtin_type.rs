//! A builtin type name where an array length belongs.
//!
//! The length slot took the right operand's tokens verbatim, so this rendered `[u8; u16]`
//! and rustc answered "expected value, found builtin type". A literal, a const generic and
//! a braced expression all stay legal.
use batch_impl::batch_impl;

#[batch_impl([u8].u16)]
trait BuiltinLength {}

fn main() {}
