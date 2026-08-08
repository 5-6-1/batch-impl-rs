//! A generator splat as a generic argument errors — its fresh declaration
//! has nowhere to live inside a `TyTypeParam`.
use batch_impl::batch_impl;

struct Pair<X, Y>(X, Y);

#[batch_impl(Pair<*(()^2)>)]
trait GenSplatArg {}

fn main() {}
