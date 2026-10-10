//! A bare fresh pack emits individual targets without discarding declarations.
//! Renaming the two fresh parameters cannot make these impl heads distinct.
use batch_impl::batch_impl;

#[batch_impl(*[].2)]
trait BareFresh {}

struct Pair<T, U>(T, U);

#[batch_impl(*Pair.*[self,Vec].*[].2)]
trait BarePairs {}

fn main() {}
