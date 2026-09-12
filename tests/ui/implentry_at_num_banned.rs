use batch_impl::batch_impl;

// `@N` needs fresh generics: no generator in this spec → `@0` is out of range.
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
