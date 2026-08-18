use batch_impl::batch_impl;

// fn-pointer templates fall back to verbatim comparison — slots inside
// `fn(A) -> B` cannot bind.
#[batch_impl(fn(u8) -> u16 impl{fn(A) -> B} { fn call(&self, x: u8) -> u16 { self(x) } })]
trait BadFnBound {
    fn call(&self, x: u8) -> u16;
}

fn main() {}
