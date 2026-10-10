use batch_impl::batch_impl;

// Groups must not hide the 32^3 copies of this statement from the token budget.
#[batch_impl((u8,).32 impl{(A@..,)} {
    fn count() -> usize {
        let mut n = 0;
        @(@(@({ n += 1; })..)..)..
        n
    }
})]
trait Repeated {
    fn count() -> usize;
}

fn main() {}
