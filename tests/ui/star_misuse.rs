//! A pack prefix requires an operand.
use batch_impl::batch_impl;

#[batch_impl(*)]
trait StarMisuse {}

fn main() {}
