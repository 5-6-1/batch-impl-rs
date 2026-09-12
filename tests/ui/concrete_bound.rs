use batch_impl::batch_impl;

// Bounds (`T: Clone`) are only valid on a trait path, in a generic
// declaration or in a bound — a concrete type's args are a plain type list.
struct Wrap<X>(X);

#[batch_impl(Wrap<u8: Clone>)]
trait BadBound {}

fn main() {}
