//! An empty `<>` is the sync marker: it fills from the spec's trait arguments.
//! On an ident that is not the annotated trait, with no arguments to fill from, the
//! brackets used to be dropped (`Vec<>` -> `Vec`) and rustc reported E0107 about a
//! *bare* `Vec` - pointing at nothing.
//!
//! No switch template appears below: the sync reaches the target type unconditionally
//! (only *body* sync needs `impl{Tr<>}`), which is the spelling a probe reached.
use batch_impl::batch_impl;

#[batch_impl([Vec<>])]
trait Tr {
    fn f();
}

// The same defect without the bracket group, so both target routes are locked.
#[batch_impl(Vec<>)]
trait Vs {
    fn f();
}

fn main() {}
