// Error: the range is empty (start is not less than end), no impl will be generated
use batch_impl::batch_impl;

#[batch_impl(()^3..2)]
trait EmptyRange {}

fn main() {}
