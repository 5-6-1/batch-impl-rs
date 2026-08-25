use batch_impl::batch_impl;

// `@5` indexes the 5th generated generic, but `().2` generates only two —
// must error at the DSL layer instead of leaking an internal reserved ident into
// rustc's E0412 output.
#[batch_impl((().2).Box<@5>)]
trait BadNum {}

fn main() {}
