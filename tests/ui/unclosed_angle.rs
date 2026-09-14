// `<` must pair with a `>`: the pairing pass runs at the entry, and an
// unclosed angle bracket is reported there rather than left for rustc (the
// `>` of `->` does not count as a closer).
use batch_impl::batch_impl;

#[batch_impl(Vec<u8)]
trait UnclosedAngle {}

fn main() {}
