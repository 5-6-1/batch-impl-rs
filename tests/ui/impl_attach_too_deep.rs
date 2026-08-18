use batch_impl::batch_impl;

// An impl{...} attachment chain beyond 128 levels errors (the parse-layer
// flat-chain guard counts every attachment kind).
#[batch_impl(i32 impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X}impl{X})]
trait TooDeep {}

fn main() {}

