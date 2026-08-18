use batch_impl::batch_impl;

// two segments with the same name prefix are rejected
#[batch_impl((u8, u16) impl{(A@.., A@..,)} { fn n(&self) -> usize { 0 } })]
trait BadVarSegDuplicate {
    fn n(&self) -> usize;
}

fn main() {}
