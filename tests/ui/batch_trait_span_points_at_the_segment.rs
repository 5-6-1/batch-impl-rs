//! A spec-level diagnostic carries the span of **its own segment**, not the macro's opening line.
//! Before this, a long `batch_trait!` reported every error on the `batch_trait!` line while the real
//! culprit sat a hundred lines below (alga2's report 3). The snapshot below pins the line number, so a
//! regression that collapses the spans back to the invocation is caught here. The trait is a marker so
//! that the crate's own diagnostic is the snapshot's **only** source of error.

use batch_impl::batch_trait;

struct A;
struct B;

trait Tr {}

batch_trait! {
    Tr: A;
    Tr: B;
    Tr: 7;
}

fn main() {}
