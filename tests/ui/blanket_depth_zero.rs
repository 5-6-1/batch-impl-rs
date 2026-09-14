// `#blanket` deref depth must be at least 1: `:0` delegates through zero
// derefs, which is the wrapper itself — meaningless.
use batch_impl::batch_impl;

trait Num {
    fn inc(&mut self);
}

#[batch_impl(<T: Num> T #blanket(@all_methods){Box:0})]
trait BlanketDepthZero {}

fn main() {}
