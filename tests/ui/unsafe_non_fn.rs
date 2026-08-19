// Error: `unsafe` juxtaposed with a non-fn type (forgot the `.`;
// `unsafe` can only modify fn types or be a bare impl marker `unsafe.T`)
use batch_impl::batch_impl;

#[batch_impl(unsafe Vec<u8>)]
trait T {}

fn main() {}
