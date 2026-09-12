use batch_impl::batch_impl;

// A leading `::` opens a global path, so it must be followed by a path
// segment identifier — `::<u8>` is an empty path with only args.
#[batch_impl(::<u8>)]
trait BadGlobalPath {}

fn main() {}
