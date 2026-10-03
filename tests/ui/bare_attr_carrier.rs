//! An attribute with nothing to annotate.
//!
//! The leaked-carrier check knew the pointer prefixes, `self` and a lone `where` block, so
//! a bare attribute fell through and the target rendered as `impl Tr for #[allow(...)] {}`
//! - not Rust, and rustc's parse error never names the mistake.
use batch_impl::batch_impl;

#[batch_impl(#[allow(dead_code)])]
trait BareAttr {}

fn main() {}
