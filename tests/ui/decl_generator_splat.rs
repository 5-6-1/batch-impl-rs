use batch_impl::batch_impl;

// A generator in the generic-declaration position has no carrier for its
// fresh declarations (`<*()^3>` would render the fresh tuple as a parameter
// name) — targeted error instead of garbage Rust.
#[batch_impl(<*()^3> Vec<u8>)]
trait BadDecl {}

fn main() {}
