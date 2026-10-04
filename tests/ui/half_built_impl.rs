//! An error *replaces* the impl — there is never a half-built impl next to a diagnostic.
//!
//! Round-7 probe D measured the opposite with one spec and two leaves: the first leaf's impl was
//! generated and kept, then the second leaf's codegen failure was appended, so the module came
//! out partially built. The hand-written witness makes that visible without a preview: before
//! the fix this file reported the `batch-impl:` error *and* `E0119 conflicting implementations`,
//! because the spec had already emitted `impl Tr for u8 {}`.
use batch_impl::batch_impl;

trait Tr {}

#[batch_impl([u8 where{@3: Clone}, u16])]
trait Generated {}

impl Tr for u16 {}

fn main() {}
