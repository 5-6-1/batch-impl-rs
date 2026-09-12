use batch_impl::batch_impl;

// A `for<…>` binder holds **lifetimes** (`for<'a>`); a type parameter there is a
// declaration in the wrong place. The DSL reports it — the binder used to be
// passed through verbatim, so this reached rustc as an "expected lifetime" error
// against the macro input.
#[batch_impl(for<u8> fn(u8))]
trait BadHrtb {}

fn main() {}
