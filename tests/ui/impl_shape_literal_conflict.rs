use batch_impl::batch_impl;

trait PairMark {}

// A single name cannot mean both the literal u8 and a replacement u16.
#[batch_impl((u8, u8): (u8, u16))]
impl PairMark for (u8, u8) {}

fn main() {}
