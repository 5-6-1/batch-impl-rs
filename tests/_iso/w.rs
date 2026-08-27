use batch_impl::batch_impl;
trait Wh2 { fn tag(&self) -> u32; }
trait NotClone {}
#[batch_impl(A<B> : [Box,Rc].u8 where{B: NotClone})]
impl Wh2 for A<B> { fn tag(&self) -> u32 { 1 } }
fn main() {}
