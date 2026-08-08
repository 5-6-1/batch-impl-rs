use batch_impl::batch_impl;

struct A;
struct B;

// `*(A,B)^N` — pow on a non-empty splat: flattening the Cartesian tuple
// combinations would duplicate the elements (E0119), so it is rejected
// with a dedicated diagnostic. Use `(A,B)^N` directly, or `T^*()^N`
// (empty splat) to generate fresh generic params.
#[batch_impl(*(A, B)^2)]
trait SplatPow {}

fn main() {}
