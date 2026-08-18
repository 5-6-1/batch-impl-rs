use batch_impl::batch_impl;

// a variadic segment is only legal as a tuple element inside `impl{...}`
#[batch_impl(u8 impl{A@..} { fn n(&self) -> usize { 0 } })]
trait BadVarSegOutside {
    fn n(&self) -> usize;
}

fn main() {}
