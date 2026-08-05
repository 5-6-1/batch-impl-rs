// Error: invalid comma position in `#fill` argument list (consecutive commas)
use batch_impl::batch_impl;

trait T {
    fn m(&self) -> u32;
    fn n(&self) -> u32;
}

#[batch_impl(usize #fill(m,,n){0})]
trait T {
    fn m(&self) -> u32;
    fn n(&self) -> u32;
}

fn main() {}
