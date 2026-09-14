// A spec may carry **one** top-level `{! ...}` block (the open-extension
// product or a hand-written `T {! m!{...}}`); a second one has no place to put
// its output.
use batch_impl::batch_impl;

macro_rules! m {
    ($($t:tt)*) => {};
}

#[batch_impl(u8 {! m!{a}} {! m!{b}})]
trait TwoTopLevel {}

fn main() {}
