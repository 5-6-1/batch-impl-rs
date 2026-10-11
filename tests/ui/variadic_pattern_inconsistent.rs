use batch_impl::batch_impl;

struct Pair<A, B>(A, B);

#[batch_impl((Pair<u8, u16>,) impl{(Pair<T, T>@..,)} #n{0})]
trait Inconsistent {
    fn n() -> usize;
}

fn main() {}
