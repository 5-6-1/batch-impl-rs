use batch_impl::batch_impl;

#[batch_impl(@u32..u8)]
trait Bad {}
