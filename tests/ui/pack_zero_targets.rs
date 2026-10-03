use batch_impl::batch_impl;

// A spec that expands to zero impls is a mistake, not a no-op: `*[]` is a star
// over the empty list, so it has no members to implement the trait for. The one
// deliberate empty spelling is a *row* inside a host (see dsl_pack_basic.rs).
#[batch_impl(*Vec *[])]
trait ZeroTargets {}

fn main() {}
