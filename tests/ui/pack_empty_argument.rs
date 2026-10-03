use batch_impl::batch_impl;

// An argument list cannot lose its argument: `*[]` is a star over the empty list,
// so this would emit `Vec<>` for rustc to reject with E0107 instead of saying what
// went wrong. (An empty `<>` block is the deliberate `X<>` sync spelling.)
#[batch_impl(Vec<*[]>)]
trait EmptyArgument {}

fn main() {}
