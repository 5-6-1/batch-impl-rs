use batch_impl::batch_impl;

struct Sink;
struct Outer(Sink);

impl Sink {
    fn consume(&self, _value: String) {}
}

// Each marker forwards the original argument; delegation never clones it.
#[batch_impl(Outer #delegate(consume){
    self.0.#call;
    self.0.#call;
})]
trait Consume {
    fn consume(&self, value: String);
}

fn main() {}
