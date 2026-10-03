//! An empty second axis retains the first axis's structural declarations.
use batch_impl::batch_impl;

struct Map<T, U>(T, U);

#[batch_impl((*Map *[].2 *[].0,))]
trait EmptySecondAxis {}

fn main() {}
