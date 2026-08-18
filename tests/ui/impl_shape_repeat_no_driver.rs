use batch_impl::batch_impl;

// a repeat block needs at least one `@ident` segment reference
#[batch_impl((u8, u16) impl{(A@..,)} { fn n(&self) -> usize { @(@0,).. } })]
trait BadRepeatNoDriver {
    fn n(&self) -> usize;
}

fn main() {}
