use batch_impl::batch_impl;

// `<...>` cannot start a `()` group: a tuple element must be a complete type
// (`(Box<u8>,)` is fine, `(<u8>)` / `(<Clone>,)` are not).
#[batch_impl((<u8>)^2 { fn tag(&self) {} })]
trait GroupAngle {}

fn main() {}
