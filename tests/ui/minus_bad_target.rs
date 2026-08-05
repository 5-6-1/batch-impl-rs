// Error: `-` missing an exclusion target (expected an identifier or the `@all` marker)
use batch_impl::batch_impl;

#[batch_impl(usize #fill(a, -){0})]
trait T {
    fn a(&self) -> u32;
}

fn main() {}
