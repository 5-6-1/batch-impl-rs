//! A diagnostic minted *while* a target type is rendered must reach the reader.
//!
//! Round-7 probe D measured that `[u8] * *` rendered `impl Tr for [u8; * ::core::compile_error!(
//! "…");,] {}` — the crate's own message inside an item rustc cannot parse, so the only visible
//! error was "expected expression, found `,`". The driver now collects a diagnostic that a
//! renderer wrote into the type and returns it as the whole expansion, which is the same promise
//! the two other refusals keep: an error replaces the impl.
use batch_impl::batch_impl;

#[batch_impl([u8] * *)]
trait Tr {}

fn main() {}
