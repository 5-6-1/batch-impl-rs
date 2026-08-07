use batch_impl::batch_impl;

// `Box:` — a colon with nothing after it must be rejected by the DSL.
#[batch_impl(#blanket(@all_methods){Box:})]
trait EmptyDepth {
    fn tag(&self);
}

fn main() {}
