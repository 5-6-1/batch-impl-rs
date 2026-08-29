use batch_impl::batch_impl;

// An empty exclusive fresh range in a where predicate must error like the
// type-position path (`@2..1` / `@2..2` cover no fresh) — it must not leak a
// raw `@` into the rendered where clause.
#[batch_impl(().2 where{@2..2: Clone})]
trait WhereEmptyExclusive {}

fn main() {}
