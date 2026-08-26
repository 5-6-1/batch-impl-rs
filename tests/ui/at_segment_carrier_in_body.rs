use batch_impl::batch_impl;

// the segment-slot carrier spelling is gone: a body-side non-fresh `@{...}`
// errors with guidance (the repeat expansion splices elements directly)
#[batch_impl((u8, u16) impl{(A@..,)} { fn f(&self) -> u8 { let x: @{A_0} = 0; x } })]
trait BadCarrierInBody {
    fn f(&self) -> u8;
}

fn main() {}
