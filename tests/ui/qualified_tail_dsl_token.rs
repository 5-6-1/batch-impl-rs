// Projection arguments use ordinary DSL validation: references need a generator.
use batch_impl::batch_impl;

struct Holder<T>(T);

#[batch_impl(Holder<<u8 as Tr>::Assoc<@0>>)]
trait Q {}

#[batch_impl(Holder<<u8 as Tr>::Assoc<@1_0>>)]
trait QGroup {}

fn main() {}
