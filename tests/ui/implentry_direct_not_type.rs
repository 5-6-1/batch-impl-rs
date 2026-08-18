use batch_impl::batch_impl;

// The direct form needs a standard Rust type (DSL operators are rejected);
// a `^` here is not the matrix separator — write the shape form with `:`.
#[batch_impl(Box^u8)]
impl BadDirect for W {
    fn mk() -> W {
        W::default()
    }
}

trait BadDirect {
    fn mk() -> Self;
}

fn main() {}
