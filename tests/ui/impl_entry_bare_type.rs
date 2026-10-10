use batch_impl::batch_impl;

trait Mark {}

#[batch_impl(u16)]
impl Mark for u8 {}

#[batch_impl(<T> Vec<T>)]
impl Mark for Vec<T> {}

fn main() {}
