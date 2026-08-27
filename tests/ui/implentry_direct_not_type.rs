use batch_impl::batch_impl;

// The direct form needs at least one type after the generic declaration
// (DSL operators are legal there — a generator may appear).
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
