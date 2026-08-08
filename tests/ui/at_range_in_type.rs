use batch_impl::batch_impl;

// `@N..M` range references are only valid as a where-predicate subject —
// in a type they error at the parse layer with a targeted message.
#[batch_impl(Vec<@0..=2>)]
trait BadRangeInType {}

fn main() {}
