// Error: `#delegate` used on a const item, should error "methods only"
use batch_impl::batch_impl;

trait HasConst {
    const VALUE: u32;
}

#[batch_impl(usize #delegate(VALUE){0})]
trait HasConst {
    const VALUE: u32;
}

fn main() {}
