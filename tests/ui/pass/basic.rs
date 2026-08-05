// A normal pass case that compiles: keeps trybuild's pass path non-empty.
use batch_impl::batch_impl;

#[batch_impl(usize, isize)]
trait Numeric {}

#[test]
fn numeric_for_usize_and_isize() {
    fn check<T: Numeric>(_: &T) {}
    check(&0usize);
    check(&0isize);
}

fn main() {}
