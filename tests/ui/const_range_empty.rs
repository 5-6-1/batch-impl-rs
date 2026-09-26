use batch_impl::batch_impl;

#[batch_impl(@u8..u8)]
trait EqualEndpoints {}

#[batch_impl(@..u8)]
trait BeforeMinimum {}

fn main() {}
