// Error: a raw number as a left operand (forbidden by the DSL)
use batch_impl::batch_impl;

trait T {}

// `0^T` → a number on the left is not allowed by the DSL rules
#[batch_impl(0^T)]
trait T {}

fn main() {}
