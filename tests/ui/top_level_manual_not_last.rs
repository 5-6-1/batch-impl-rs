use batch_impl::batch_impl;

// A `{! ...}` block followed by a `{...}` block is illegal — the top-level
// macro form must be the last block. Under the current block order the chain
// may take the top-level path (the nonexistent macro then errors at rustc)
// or walk_top_level's "must be the last block" path — either way the writing
// must not compile.
#[batch_impl(u32 {! my_macro!{}} {fn other() {}})]
trait BadChain {
    fn add(&self) -> Self;
}

fn main() {}
