use batch_impl::batch_impl;

// A `#` followed by a non-bracket group cannot open a block, so the return
// expression's token fold used to spin forever on an unmoved cursor — no
// allocation growth, so the fuzz guard could not catch it and the compiler
// simply hung. It reports the stalled token now.
#[batch_impl(extern "C" fn(u8) -> u8 #(x))]
trait StrayHash {}

fn main() {}
