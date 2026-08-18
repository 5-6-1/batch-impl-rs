use batch_impl::batch_impl;

// a repeat block referencing a segment the template does not declare
#[batch_impl((u8, u16) impl{(A@..,)} { fn n(&self) -> usize { @(@X::f(),).. } })]
trait BadRepeatUnknown {
    fn n(&self) -> usize;
}

fn main() {}
