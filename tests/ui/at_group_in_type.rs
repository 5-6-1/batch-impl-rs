use batch_impl::batch_impl;

// `@2_0` refers to group 2, which the spec's generators never created
// (`()^3-()^3` has groups 0 and 1) — must error at the DSL layer instead of
// leaking the reserved `_Param_2_0_BatchGen_` name into rustc's E0412 output.
#[batch_impl((()^3-()^3)^Box<@2_0>)]
trait BadGroupType {}

fn main() {}
