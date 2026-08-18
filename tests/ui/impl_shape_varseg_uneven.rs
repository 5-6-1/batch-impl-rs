use batch_impl::batch_impl;

// two same-level segments need an evenly split leaf (3 elements cannot be
// split across 2 segments)
#[batch_impl((u8, u16, u32) impl{(A@.., B@..,)} { fn n(&self) -> usize { 0 } })]
trait BadVarSegUneven {
    fn n(&self) -> usize;
}

fn main() {}
