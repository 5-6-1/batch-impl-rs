//! The reported case exactly: the text before `:` is the **trait's own name** on an impl entry.
//! 0.8.0 – 0.9.5 rejected it loudly ("the impl's for-Type must match the shape template
//! ident-for-ident"); `261dd35` (released as 0.9.6) deleted that guard together with the
//! for-type/template identity rule, and from then on the slot matched an ident, rewrote nothing and
//! emitted the input impl unchanged — silently. This snapshot pins the restored diagnostic.

use batch_impl::batch_impl;

struct A;
struct B;

trait T {
    fn t(&self) -> u8;
}

#[batch_impl(T: A, B)]
impl T for A {
    fn t(&self) -> u8 {
        0
    }
}

fn main() {}
