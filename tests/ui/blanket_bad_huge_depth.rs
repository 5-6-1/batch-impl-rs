use batch_impl::batch_impl;

// `Box:999999` — an unbounded deref depth would expand into a pathological
// type and overflow rustc; capped at MAX_BLANKET_DEPTH (128).
#[batch_impl(#blanket(@all_methods){Box:999999})]
trait HugeDepth {
    fn tag(&self);
}

fn main() {}
