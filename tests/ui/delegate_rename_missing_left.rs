// A `#delegate` rename whose left side is missing (`=foo`) is a user error,
// not a panic: the `eq - 1` lookup used to underflow in debug builds
// (no-panic promise; fuzz cannot reach this shape reliably).
use batch_impl::batch_impl;

#[batch_impl(Wrapper<T> #delegate(=foo){self.0})]
trait D {}
