// Error: `,,` consecutive commas and leading commas (missing operands around the separator)
use batch_impl::batch_impl;

#[batch_impl(,usize)]
trait LeadComma {}

#[batch_impl(usize,,isize)]
trait DoubleComma {}

fn main() {}
