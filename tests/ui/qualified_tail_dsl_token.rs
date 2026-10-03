// Error: a `::`-tail segment is plain Rust path text — a DSL token there is
// reported instead of leaking into the rendered impl (`Assoc<@0>` would reach
// rustc as "expected type, found `@`").
use batch_impl::batch_impl;

struct Holder<T>(T);

#[batch_impl(Holder<<u8 as Tr>::Assoc<@0>>)]
trait Q {}

// The same guard covers a `*` pack prefix — `Assoc<*[A, B]>` used to leak the star
// into the rendered path. (A `*` that names its pointee, `*const T` / `*mut T`, is
// ordinary Rust and stays legal in a tail.)
#[batch_impl(Holder<<u8 as Tr>::Assoc<*[A, B]>>)]
trait QStar {}

fn main() {}
