use batch_impl::batch_impl;

#[batch_impl(#blanket(@all){Box:abc})]
trait BadDepth {
    fn m(&self) -> u32;
}
