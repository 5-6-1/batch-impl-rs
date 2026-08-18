use batch_impl::batch_impl;

// a bare `@` in an impl body (pattern syntax is not DSL here)
#[batch_impl((u8, u16) impl{(A@..,)} { fn n(&self) -> usize { let x = 0; x @ 0 } })]
trait BadRepeatBareAt {
    fn n(&self) -> usize;
}

fn main() {}
