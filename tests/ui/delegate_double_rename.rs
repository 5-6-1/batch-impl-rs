// Error: the same trait method is renamed twice (`size=len, size=other`) —
// a method can delegate to only one target method.
use batch_impl::batch_impl;

struct Inner;
impl Inner {
    fn len(&self) -> usize {
        5
    }
    fn other(&self) -> usize {
        9
    }
}

struct Wrap(Inner);

#[batch_impl(Wrap #delegate(size=len, size=other){self.0})]
trait HasSize {
    fn size(&self) -> usize;
}

fn main() {}
