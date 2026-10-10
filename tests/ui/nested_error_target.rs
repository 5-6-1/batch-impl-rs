//! A nested DSL diagnostic must survive conversion of an array length to tokens.
//!
//! Round-7 probe D measured that `[u8] * *` rendered `impl Tr for [u8; * ::core::compile_error!(
//! "…");,] {}` — the crate's own message inside an item rustc cannot parse, so the only visible
//! error was "expected expression, found `,`". The array apply now collects a structural error
//! before it becomes an opaque length and returns it as the whole expansion, the same promise
//! the two other refusals keep: an error replaces the impl.
use batch_impl::batch_impl;

#[batch_impl([u8] * *)]
trait Tr {}

fn main() {}
