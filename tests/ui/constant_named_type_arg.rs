//! The type below is deliberately lower-case (`constant`), so the camel-case
//! lint is allowed for this file only — the name *is* the fixture.
#![allow(non_camel_case_types)]

use batch_impl::batch_impl;

// A type literally named `constant` used as a *type argument*: `<constant>` is
// an argument list, not a generic declaration (only a bound — or the `const`
// keyword — makes a declaration). The name-prefix check in `is_declaration`
// must not treat it as a `const` parameter.
struct constant;

#[batch_impl(Vec.<constant>)]
trait Held {}

fn main() {
    // A `pass` fixture compiles either way, so "it compiles" alone would not
    // notice a silent 0-impl expansion (which is what the bug produced before:
    // `<constant>` hoisted as a declaration → E0107). Assert the impl exists.
    fn need<T: Held>() {}
    need::<Vec<constant>>();
}
