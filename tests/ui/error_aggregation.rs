use batch_impl::batch_impl;

// Two independent spec errors must be reported together (error aggregation
// collects every spec's error instead of stopping at the first). Both specs
// fail at parse time (a number on the left); `@0..=2` is no longer an error
// here — range references now fold at parse and re-open at render.
#[batch_impl(0.T, 1.U)]
trait AggT {}

fn main() {}
