use batch_impl::batch_impl;

// Bindings (`Item = u32`) are only valid on a trait path or in a generic
// declaration — a concrete type's args are a plain type list.
struct Assoc<T> {
    v: T,
}

#[batch_impl(Assoc<Item = u32>)]
trait BadBinding {}

fn main() {}
