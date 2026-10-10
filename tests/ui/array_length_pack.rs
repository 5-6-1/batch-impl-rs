//! Array lengths are Rust expressions: these type names cannot be used as values.
//! The same leading `*` and `[]` tokens can start legal Rust const expressions.
use batch_impl::batch_impl;

#[batch_impl([u8; *[u8, u16]])]
trait PackLength {}

#[batch_impl([u8; [u8, u16]])]
trait ListLength {}

fn main() {}
