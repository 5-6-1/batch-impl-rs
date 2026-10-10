use batch_impl::batch_impl;

// Every nonempty impl spec requires an explicit template and source.
#[batch_impl(<T>)]
impl BadDirect for W {
    fn mk() -> W {
        W::default()
    }
}

trait BadDirect {
    fn mk() -> Self;
}

fn main() {}
