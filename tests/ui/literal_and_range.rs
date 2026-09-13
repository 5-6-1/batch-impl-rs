use batch_impl::batch_impl;

// Non-integer literals and non-integer range endpoints are not types.
// (Both used to report the *depth guard* instead: the literal block's error
// paths left the cursor in place, so the space-application chain folded the
// same token up to the 129-level cap and reported that.)
#[batch_impl(1.5)]
trait LitT {}

#[batch_impl(1..x)]
trait RangeT {}

fn main() {}
