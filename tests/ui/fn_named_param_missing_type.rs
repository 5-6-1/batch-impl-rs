use batch_impl::batch_impl;

// A named fn parameter without a type: the name is consumed as a prefix, so
// the missing type is reported rather than rendered as `x:` for rustc.
#[batch_impl(fn(x:) -> u8)]
trait BadNamedParam {}

fn main() {}
