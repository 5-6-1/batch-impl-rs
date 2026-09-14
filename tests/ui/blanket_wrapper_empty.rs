// `#blanket`'s wrapper list is `,`-separated: a trailing comma (or a leading
// one) is an empty element, not a wrapper with no type.
use batch_impl::batch_impl;

trait Num {
    fn inc(&mut self);
}

#[batch_impl(<T: Num> T #blanket(@all_methods){Box,})]
trait BlanketWrapperEmpty {}

fn main() {}
