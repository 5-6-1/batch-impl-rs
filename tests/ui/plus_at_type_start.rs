// A `+` cannot start a type — it belongs in a bound (`T: Clone + Send`).
// Without the targeted diagnostic this spec silently generated 0 impls.
use batch_impl::batch_impl;

#[batch_impl(+A)]
trait PlusStart {
    fn m(&self) -> u8;
}

fn main() {}
