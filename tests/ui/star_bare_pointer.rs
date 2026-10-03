use batch_impl::batch_impl;

// `*const` / `*mut` without a pointee is a prefix still waiting for a type: it used
// to render `impl BarePointer for *const {}` for rustc to reject.
#[batch_impl(*const)]
trait BarePointer {}

fn main() {}
