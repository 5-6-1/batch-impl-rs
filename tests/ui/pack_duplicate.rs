//! Identical materialized trait impl heads receive the definite-duplicate diagnostic.
use batch_impl::batch_impl;

#[batch_impl(*[u8, u8])]
trait DuplicateTargets {}

fn main() {}
