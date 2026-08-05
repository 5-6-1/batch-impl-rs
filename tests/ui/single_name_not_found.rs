// Error: the item referenced by `#name{body}` is not in the trait
use batch_impl::batch_impl;

trait T {
    fn m(&self) -> u32;
}

#[batch_impl(usize #no_such{0})]
trait T {
    fn m(&self) -> u32;
}

fn main() {}
