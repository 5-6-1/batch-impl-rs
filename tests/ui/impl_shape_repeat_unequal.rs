use batch_impl::batch_impl;

// segments driving one block must be equal-length (A len 2, B len 3)
#[batch_impl(
    ((u8, u16), (u32, u64, u128)) impl{((A@..,),(B@..,))}
    { fn n(&self) -> usize { @(@A::f() @B::g(),).. } }
)]
trait BadRepeatUnequal {
    fn n(&self) -> usize;
}

fn main() {}
