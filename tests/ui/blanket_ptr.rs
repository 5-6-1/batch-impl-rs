use batch_impl::batch_impl;

#[batch_impl(#blanket(#all){*const})]
trait BadPtr {
    fn m(&self) -> u32;
}
