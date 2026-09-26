//! A bare fresh pack emits individual targets without discarding declarations.
use batch_impl::batch_impl;

#[batch_impl(*().2)]
trait BareFresh {}

fn main() {}
