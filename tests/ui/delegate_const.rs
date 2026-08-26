use batch_impl::batch_impl;

// `#delegate` is methods-only: a trait const is not delegable (there is no
// value path to forward — the inner would need its own inherent const, and
// the directive's body expression targets receivers).
#[batch_impl(ConstWrap #delegate(@all){self.0})]
trait ConstApi {
    const LIMIT: u32;
}

struct ConstWrap;

fn main() {}
