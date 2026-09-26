use batch_impl::batch_impl;

struct Wrap<const N: usize>([u8; N]);
trait Width {}

// A declared const parameter cannot bind a type argument.
#[batch_impl(Wrap<N>: Wrap<u8>)]
impl<const N: usize> Width for Wrap<N> {}

fn main() {}
