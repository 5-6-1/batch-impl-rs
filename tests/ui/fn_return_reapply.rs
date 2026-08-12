use batch_impl::batch_impl;

// `fn(A) -> B - C`: `-> B` already fills the return type; the trailing `- C`
// applies again and must error with guidance (not silently drop the `- C`).
#[batch_impl(fn(A) -> B - C)]
trait FnReturnReapply {}

fn main() {}
