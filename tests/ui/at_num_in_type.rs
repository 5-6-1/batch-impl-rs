use batch_impl::batch_impl;

// `@5` indexes the 5th generated generic, but `().2` generates only two —
// must error at the DSL layer instead of leaking `_Param_5_BatchGen_` into
// rustc's E0412 output.
#[batch_impl((().2).Box<@5>)]
trait BadNum {}

fn main() {}
