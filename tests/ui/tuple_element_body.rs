//! A `{body}` written on a tuple element.
//!
//! The bracket path rejects this (a per-element body needs a *list*), but the tuple branch
//! had no such guard, so the body was rendered inside the element's type:
//! `impl Tr for (u8 { fn tag(& self) -> u8 { 1 } }, u16,)`. That is not Rust, and rustc's
//! parse error never mentions the attribute.
use batch_impl::batch_impl;

#[batch_impl((u8 { fn tag(&self) -> u8 { 1 } }, u16))]
trait TupleElementBody {
    fn tag(&self) -> u8;
}

fn main() {}
