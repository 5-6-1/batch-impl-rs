use batch_impl::{batch_impl, batch_preprocess_test};

// A top-level `{! ...}` block (the `#cmd` open-extension product) must be
// the last block — a following `{...}` body errors.
#[batch_impl(usize #batch_preprocess_test(add,inc){*self+1} {fn other() {}})]
trait BadChain {
    fn add(&self) -> Self;
    fn inc(&self) -> Self;
}

fn main() {}
