use batch_impl::batch_impl;

// Non-integer literals and non-integer range endpoints are not types.
#[batch_impl(1.5)]
trait LitT {}

#[batch_impl(1..x)]
trait RangeT {}

fn main() {}
