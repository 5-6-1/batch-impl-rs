use batch_impl::batch_impl;

// A splat as a where-predicate subject has no defined semantics
// (`*(A,B): Trait` would have to mean `A: Trait, B: Trait` — a predicate is a
// constraint, not a parameter list). The macro rejects it with a clear
// message. Wrapping it (`(*(A,B)): Trait`) does not help: nothing expands a
// splat inside a predicate, so the pipeline's final predicate check reports
// that form too.
#[batch_impl(u8 where{*(A, B): Clone})]
trait WhereSplatBad {}

fn main() {}
