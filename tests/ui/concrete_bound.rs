use batch_impl::batch_impl;

// Bounds (`T: Clone`) are only valid on a trait path or in a generic
// declaration — a concrete type's args are a plain type list.
struct Wrap<X>(X);

#[batch_impl(Wrap<u8: Clone>)]
trait BadBound {}

fn main() {}
