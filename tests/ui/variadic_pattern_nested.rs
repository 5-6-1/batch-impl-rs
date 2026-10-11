use batch_impl::batch_impl;

#[batch_impl(((u8,),) impl{((T@..,)@..,)} #n{0})]
trait Nested {
    fn n() -> usize;
}

fn main() {}
