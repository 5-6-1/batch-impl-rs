use batch_impl::batch_impl;

struct A;
struct B;

#[batch_impl(A B)]
trait AdjacentTypes {}

fn main() {}
