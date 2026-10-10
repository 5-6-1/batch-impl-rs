use batch_impl::batch_impl;

struct Holder<T>(T);
struct NoBound;
trait Bound {}
trait Nested {}
trait Outer {}
trait Handwritten {}

#[batch_impl(X: Holder<(<T: Bound> T)>)]
impl<T> Nested for X {}

#[batch_impl(X: <T: Bound> Holder<T>)]
impl<T> Outer for X {}

impl<T: Bound> Handwritten for Holder<T> {}

fn need_nested<T: Nested>() {}
fn need_outer<T: Outer>() {}
fn need_handwritten<T: Handwritten>() {}

fn main() {
    need_nested::<Holder<NoBound>>();
    need_outer::<Holder<NoBound>>();
    need_handwritten::<Holder<NoBound>>();
}
