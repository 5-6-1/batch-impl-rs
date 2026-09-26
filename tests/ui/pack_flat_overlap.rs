//! Flattened two-axis length families can overlap despite distinct dimensions.
use batch_impl::batch_impl;

struct Map<T, U>(T, U);

#[batch_impl((*Map *().1..=2 *().1..=3,))]
trait FlatFamily {}

fn main() {}
