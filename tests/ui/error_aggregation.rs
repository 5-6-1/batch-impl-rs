use batch_impl::batch_impl;

// Two independent spec errors must be reported together (error aggregation
// collects every spec's error instead of stopping at the first).
#[batch_impl(0^T, @0..=2)]
trait AggT {}

fn main() {}
