//! The same rule on the **body** token path: body sync is opt-in through a switch template
//! (`impl{Tr<>}`), and the `X<>` it then meets has nothing to fill from either.
use batch_impl::batch_impl;

#[batch_impl(u8 #tag3{X<>} impl{Tr<>})]
trait BodySurface {
    fn tag3(&self) -> u32;
}

fn main() {}
