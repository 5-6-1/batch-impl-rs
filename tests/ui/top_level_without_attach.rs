use batch_impl::batch_impl;

// A standalone `{! ...}` block (no attached type) has no spec body to
// prepend to the macro input — error instead of emitting invalid Rust.
#[batch_impl({! some_macro!{}})]
trait NoAttach {}

fn main() {}
