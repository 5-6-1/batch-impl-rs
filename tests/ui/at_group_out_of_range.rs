use batch_impl::batch_impl;

// `@2_0` refers to group 2, which the spec's generators never created
// (`().3-().3` has groups 0 and 1) — must error at the DSL layer.
#[batch_impl(().3 ().3 where{@2_0: Clone})]
trait BadGroup {}

fn main() {}
