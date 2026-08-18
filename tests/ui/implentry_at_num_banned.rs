use batch_impl::batch_impl;

// `@N` position references are banned on the ItemImpl entry.
#[batch_impl(W : Box<@0>)]
impl BadAtN for W {
    fn mk() -> W {
        W::default()
    }
}

trait BadAtN {
    fn mk() -> Self;
}

fn main() {}
