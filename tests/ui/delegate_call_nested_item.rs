use batch_impl::batch_impl;

struct Inner;
struct Outer(Inner);

impl Inner {
    fn value(&self) -> usize {
        1
    }
}

// A nested item does not inherit the enclosing method's call marker.
// Rust diagnoses its unchanged marker instead of silently delegating it.
#[batch_impl(Outer #delegate(value){
    fn nested(inner: &Inner) -> usize {
        inner.#call
    }
    self.0.#call
})]
trait Value {
    fn value(&self) -> usize;
}

fn main() {}
