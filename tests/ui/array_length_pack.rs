//! An array length takes a const expression, not a DSL position: a pack or a
//! bracket list there used to render `[u8; * [u8, u16]]` and reach rustc as E0423,
//! with nothing pointing at the attribute.
use batch_impl::batch_impl;

#[batch_impl([u8; *[u8, u16]])]
trait PackLength {
    fn pack_length(&self) -> u8;
}

#[batch_impl([u8; [u8, u16]])]
trait ListLength {
    fn list_length(&self) -> u8;
}

fn main() {}
