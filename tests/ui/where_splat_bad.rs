use batch_impl::batch_impl;

// A bare splat as a where-predicate subject has no defined semantics
// (`*(A,B): Trait` would expand to `A, B: Trait` — a predicate is a
// constraint, not a parameter list). The macro rejects it with a clear
// message; wrap in a tuple (`(*(A,B)): Trait`) or write separate
// predicates.
#[batch_impl(u8 where{*(A, B): Clone})]
trait WhereSplatBad {}

fn main() {}
