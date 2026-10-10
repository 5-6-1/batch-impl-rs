use batch_impl::batch_impl;

trait Empty {}

#[batch_impl(Vec<u8>:)]
impl Empty for u16 {}

#[batch_impl(@Self: <T>)]
impl Empty for Vec<T> {}

#[batch_impl(Unused where{u8: Copy}:)]
impl Empty for u32 {}

fn main() {}
