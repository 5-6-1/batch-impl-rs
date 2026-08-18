use batch_impl::batch_impl;

// Only `@trait` is allowed on the ItemImpl entry — `@` constants are banned.
#[batch_impl(W : @u*)]
impl BadConst for W {
    fn mk() -> W {
        W::default()
    }
}

trait BadConst {
    fn mk() -> Self;
}

fn main() {}
