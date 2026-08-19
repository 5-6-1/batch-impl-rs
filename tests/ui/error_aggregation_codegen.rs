use batch_impl::batch_impl;

// Two independent codegen-stage errors (where-predicate `@N` out of range)
// must be reported together — the driver calls generate_impl for every spec
// without short-circuiting.
#[batch_impl(().1-().1 where{@5: Clone}, ().2 where{@3: Copy})]
trait AggCg {}

fn main() {}
