//! Specs that cannot start are refused without consuming anything, so they used to vanish.
//!
//! Both shapes matter: a refused *first* spec ends the loop before it ever runs, and a
//! refused *later* spec ends it with an earlier impl already emitted - so the module compiles
//! partially, and `docs/reference.md:1060` promises the opposite.
use batch_impl::batch_impl;

#[batch_impl(^u8)]
trait DroppedAlone {}

#[batch_impl(u8, ^u16)]
trait DroppedSecond {}

#[batch_impl(u8: ^u8)]
impl DroppedEntry for u8 {}

trait DroppedEntry {}

fn main() {}
