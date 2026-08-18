use batch_impl::batch_impl;

// An `impl{...}` template holds a standard Rust type — DSL operators are
// rejected by syn parsing, not silently interpreted.
#[batch_impl(i32 impl{A^B})]
trait BadTemplate {}

fn main() {}
