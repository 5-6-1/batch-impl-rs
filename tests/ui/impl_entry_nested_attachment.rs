use batch_impl::batch_impl;

struct Wrap<T>(T);
trait Mark {}

#[batch_impl(Target: Wrap<u8 impl{A}>)]
impl Mark for Target {}

#[batch_impl(Target: (u8 impl{fn},))]
impl Mark for Target {}

fn main() {}
