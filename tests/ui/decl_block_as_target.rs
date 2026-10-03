//! A `<>` declaration block used as the whole target.
//!
//! `<T> Vec<T>` declares a parameter and applies it, which is the documented use; `<T>`
//! alone declares one and applies it to nothing, so the target is not a type. It used to
//! render `impl Tr for <T> {}`, whose rustc parse error carries no `batch-impl:` prefix.
use batch_impl::batch_impl;

#[batch_impl(<T>)]
trait DeclBlockAsTarget {}

fn main() {}
