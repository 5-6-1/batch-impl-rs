//! An empty `<>` is the sync marker: it fills from the spec's trait arguments.
//! On an ident that is not the annotated trait, with no arguments to fill from, the
//! brackets used to be dropped (`Vec<>` -> `Vec`) and rustc reported E0107 about a
//! *bare* `Vec` - pointing at nothing.
//!
//! The switch template is what makes the sync run for the target type.
use batch_impl::batch_impl;

#[batch_impl(Vec<> impl{Tr<>})]
trait Tr {
    fn f();
}

fn main() {}
