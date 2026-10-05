//! A **bare-ident template** on the impl entry that names nothing the shape could rewrite: the slot
//! matched structurally, rewrote nothing, and every leaf silently emitted the input impl unchanged.
//! The reported shape was `#[batch_impl(T: A, B; U: C, D)]` — on the impl entry the text before `:` is
//! a *template*, not a trait name, and the trait comes from the input impl (alga2's report, round 12).

use batch_impl::batch_impl;

struct A;
struct B;

trait T {
    fn t(&self) -> u8;
}

#[batch_impl(NotTheSelfType: A, B)]
impl T for A {
    fn t(&self) -> u8 {
        0
    }
}

fn main() {}
