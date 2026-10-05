//! A spec whose leaf is **ignored**: the template binds nothing that occurs in the impl, and the produced
//! for-type is not the leaf either — so the impl came out for the input's self type while the leaf was
//! dropped. Measured before this diagnostic existed: `Vec<u8> : Vec<u8>` on `impl Make for u8` produced
//! an impl for `u8` and nothing else, with no message at all (reported from an independent analysis pass).
//! The sibling case — a bare-ident template naming nothing — is `impl_entry_template_describes_nothing`.
//!
//! The four shapes are named on purpose: the older arity diagnostic never said what the input self type
//! looked like, so a reader could not tell which of the three types to change.

use batch_impl::batch_impl;

trait Make {
    fn make() -> Self;
}

#[batch_impl(Vec<u8> : Vec<u8>)]
impl Make for u8 {
    fn make() -> Self {
        0
    }
}

fn main() {}
