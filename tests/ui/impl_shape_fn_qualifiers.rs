use batch_impl::batch_impl;

trait Callable {}

// Matching a function prototype never erases the unsafe calling contract.
#[batch_impl(fn(u8): unsafe fn(u16))]
impl Callable for fn(u8) {}

fn main() {}
