use batch_impl::batch_impl;

// A `Self` **inside a group** (`(Self, u8)`) cannot be blanket-delegated
// either: the forwarded call passes the inner type where the wrapper's
// `Self` is expected. The bare-Self detection must recurse into groups —
// a top-level-only scan would miss it and emit a delegation that fails
// with a confusing rustc E0308.
#[batch_impl(#blanket(@all_ref_methods){Box})]
trait GroupSelf {
    fn f(&self, x: (Self, u8));
}

fn main() {}
