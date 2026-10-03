use batch_impl::batch_impl;

// `*` needs a type operand. A literal would otherwise be packed as one member and
// leak `impl StarLiteral for 1 {}` into the output.
#[batch_impl(*1)]
trait StarLiteral {}

fn main() {}
