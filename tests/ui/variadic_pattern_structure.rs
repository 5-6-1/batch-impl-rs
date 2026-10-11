use batch_impl::batch_impl;

#[batch_impl((u8,) impl{(A<B>@..,)} #n{0})]
trait Structure {
    fn n() -> usize;
}

#[batch_impl((Vec<u8>,) impl{(A<B, C>@..,)} #n{0})]
trait GenericArity {
    fn n() -> usize;
}

fn main() {}
