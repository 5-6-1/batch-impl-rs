use batch_impl::batch_impl;

// a declared driver (`@A(...)..`) conflicts with an inner reference to a
// different segment
#[batch_impl(
    ((u8, u16), (u32, u64)) impl{((A@..,),(B@..,))}
    { fn n(&self) -> usize { (@A(@B::f(),)..) } }
)]
trait BadDriverConflict {
    fn n(&self) -> usize;
}

fn main() {}
