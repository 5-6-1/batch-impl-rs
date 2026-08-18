use batch_impl::batch_impl;

struct Additive;

// `X<>` for a trait that is not the spec's trait errors — empty brackets
// are only the spec's own trait application
#[batch_impl(Semiring<Additive> ()^1..=1 where{@0..: Other<>})]
trait Semiring<Oa> {}

fn main() {}
