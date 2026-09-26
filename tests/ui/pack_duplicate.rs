//! Pack materialization never silently deduplicates overlapping targets.
use batch_impl::batch_impl;

#[batch_impl(*(u8, u8))]
trait DuplicateTargets {}

fn main() {}
