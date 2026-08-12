use batch_impl::batch_impl;

// A static method returning `Self` cannot be blanket-delegated: `t::new()`
// returns the inner type, not the wrapper's `Self`. Guided error instead of
// rustc's E0308 mismatched types at the generated impl.
#[batch_impl(#blanket(@all_static_methods){Box})]
trait NewT {
    fn new() -> Self;
}

fn main() {}
