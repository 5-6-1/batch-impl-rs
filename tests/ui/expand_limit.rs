// Error: `^N` expansion product count exceeds the limit of 1024, treated as a typo
use batch_impl::batch_impl;

#[batch_impl(()^2000)]
trait TooMany {}

fn main() {}
