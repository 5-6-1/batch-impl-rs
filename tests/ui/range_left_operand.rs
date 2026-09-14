// A numeric range is a **list** of impl counts, so it belongs on the right of
// an apply (`T.0..3`), never as the left operand.
use batch_impl::batch_impl;

#[batch_impl(0..3.u8)]
trait RangeLeft {}

fn main() {}
