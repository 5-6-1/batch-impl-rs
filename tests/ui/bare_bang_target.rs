//! `!` as the whole target.
//!
//! Nested inside a type it is the never return type and stays legal (see
//! tests/features/dsl_basic.rs); as the target itself it reached rustc as an item that does
//! not parse, with no `batch-impl:` prefix on the error.
use batch_impl::batch_impl;

#[batch_impl(! u8)]
trait BareBang {}

fn main() {}
