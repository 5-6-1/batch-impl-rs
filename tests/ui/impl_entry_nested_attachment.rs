use batch_impl::batch_impl;

struct Wrap<T>(T);
trait Mark {}

#[batch_impl(Target: Wrap<u8 impl{A}>)]
impl Mark for Target {}

#[batch_impl(Target: (u8 impl{fn},))]
impl Mark for Target {}

trait Bound<T> {}

#[batch_impl(Target: Wrap<<T: Bound<u8 impl{A}>> T>)]
impl<T> Mark for Target {}

#[batch_impl(Target: Wrap<<T: Bound<u8 impl{fn}>> T> impl{Outer})]
impl<T> Mark for Target {}

#[batch_impl(Target: (*Vec *[<Bound<u8 impl{A}>>,].1,))]
impl Mark for Target {}

fn main() {}
