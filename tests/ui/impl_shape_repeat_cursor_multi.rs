use batch_impl::batch_impl;

// a cursor-only block with several template segments cannot pick a length —
// declare the driving segment instead
#[batch_impl(
    (u8, u16, u32, u32) impl{(A@.., B@..,)}
    { fn n(&self) -> usize { (@(self.@0,)..) } }
)]
trait BadCursorMulti {
    fn n(&self) -> usize;
}

fn main() {}
