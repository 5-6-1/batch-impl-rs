// Error: the kept list is empty after excluding everything (`-@all`)
use batch_impl::batch_impl;

#[batch_impl(usize #fill(@all,-@all){0})]
trait T {
    fn m(&self) -> u32;
}

fn main() {}
