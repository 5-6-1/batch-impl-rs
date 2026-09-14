// `@trait` names the trait being implemented; on an **inherent** impl
// (`impl Vec<u8> {}`) there is no trait to refer to, so it is reported at the
// constant stage rather than rendering an empty path.
use batch_impl::batch_impl;

#[batch_impl(@trait)]
impl Vec<u8> {}

fn main() {}
