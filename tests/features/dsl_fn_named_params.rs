//! Named parameters in a `fn(...)` **pointer** type — `fn(x: u8) -> u8`.
//!
//! A `fn(...)` type may name its parameters (valid Rust), but the DSL type
//! grammar had no room for `name:` and reported "unexpected `:` after the
//! type". The name is now kept verbatim as a prefix and the type after `:` is
//! parsed **structurally**, so DSL operands keep working inside a named
//! parameter (`fn(x: Box<u8>)`).
//!
//! `Fn(x: u8)` stays an error — rustc's own rule for `Trait(...)` syntax
//! ("does not support named parameters"), locked by
//! `tests/ui/fn_sugar_named_param.rs`.

use batch_impl::batch_impl;

#[batch_impl(fn(x: u8) -> u8 { fn tag(&self) -> &'static str { "named" } })]
trait NamedFn {
    fn tag(&self) -> &'static str;
}

// A DSL operand inside the named parameter: the type after `:` is parsed, not
// copied, so `Box<u8>` is a real node.
#[batch_impl(fn(v: Box<u8>) -> u8 { fn tag(&self) -> &'static str { "boxed-param" } })]
trait NamedBoxedFn {
    fn tag(&self) -> &'static str;
}

// An anonymous parameter (`fn(_: u8)`) is a named one whose name is `_`.
#[batch_impl(fn(_: u8) { fn tag(&self) -> &'static str { "anon" } })]
trait AnonParamFn {
    fn tag(&self) -> &'static str;
}

// `extern "C" fn(...)` is an opaque passthrough block — the same spelling keeps
// working there (the named parameter is never re-parsed).
#[batch_impl(extern "C" fn(v: u8) { fn tag(&self) -> &'static str { "c" } })]
trait ExternNamedFn {
    fn tag(&self) -> &'static str;
}

// The impl entry shares the type parser, so a named parameter works in its
// direct form's for-type too.
#[batch_impl(fn(w: u8) -> u8)]
impl NamedEntry for fn(u8) -> u8 {
    fn tag(&self) -> &'static str {
        "entry"
    }
}

trait NamedEntry {
    fn tag(&self) -> &'static str;
}

#[test]
fn named_fn_parameters_compile() {
    fn tag_of<T: NamedFn>(t: &T) -> &'static str {
        t.tag()
    }
    // The name is part of the type's spelling, not of its identity.
    let named: fn(x: u8) -> u8 = |x| x;
    let plain: fn(u8) -> u8 = |x| x;
    assert_eq!(tag_of(&named), "named");
    assert_eq!(tag_of(&plain), "named");

    fn tag_boxed<T: NamedBoxedFn>(t: &T) -> &'static str {
        t.tag()
    }
    let boxed: fn(v: Box<u8>) -> u8 = |b| *b;
    assert_eq!(tag_boxed(&boxed), "boxed-param");

    fn tag_anon<T: AnonParamFn>(t: &T) -> &'static str {
        t.tag()
    }
    let anon: fn(_: u8) = |_| ();
    assert_eq!(tag_anon(&anon), "anon");

    fn tag_c<T: ExternNamedFn>(t: &T) -> &'static str {
        t.tag()
    }
    extern "C" fn c_fn(v: u8) {
        let _ = v;
    }
    let c: extern "C" fn(v: u8) = c_fn;
    assert_eq!(tag_c(&c), "c");

    fn tag_entry<T: NamedEntry>(t: &T) -> &'static str {
        t.tag()
    }
    let named_entry: fn(u8) -> u8 = |x| x;
    assert_eq!(tag_entry(&named_entry), "entry");
}
