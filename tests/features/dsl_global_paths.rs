//! Leading-`::` **global paths** — `::std::vec::Vec<u8>`.
//!
//! `::` opens a block, so a global path works at the start of a spec, nested in
//! an args list (`Box<::std::vec::Vec<u8>>`), and in both front-ends (the
//! attribute entry and the impl entry). Before this, `parse_item` returned
//! `None` for a leading `::` and the spec was silently empty (nested) or
//! unparseable (top level).

use batch_impl::batch_impl;

#[batch_impl(::std::vec::Vec<u8> { fn tag(&self) -> &'static str { "vec" } })]
trait GlobalVec {
    fn tag(&self) -> &'static str;
}

// Nested in an args list: the `::` is part of the inner block.
#[batch_impl(Box<::std::vec::Vec<u8>> { fn tag(&self) -> &'static str { "boxed" } })]
trait GlobalBoxedVec {
    fn tag(&self) -> &'static str;
}

// A global path as the head of a longer path (here the `IntoIter` of a global
// `Vec`; a `::`-**tail** on a global head is covered by the parser tests — as
// plain Rust it needs the fully-qualified form rustc's E0223 asks for).
#[batch_impl(::std::vec::IntoIter<u8> { fn tag(&self) -> &'static str { "into-iter" } })]
trait GlobalIntoIter {
    fn tag(&self) -> &'static str;
}

// The impl entry shares the target parser: the direct form's for-type is a DSL
// type, so the same spelling works there.
#[batch_impl(::core::option::Option<u8>)]
impl GlobalEntry for Option<u8> {
    fn tag(&self) -> &'static str {
        "option"
    }
}

trait GlobalEntry {
    fn tag(&self) -> &'static str;
}

#[test]
fn global_path_targets_compile() {
    fn tag_of<T: GlobalVec>(t: &T) -> &'static str {
        t.tag()
    }
    assert_eq!(tag_of(&Vec::<u8>::new()), "vec");

    fn tag_boxed<T: GlobalBoxedVec>(t: &T) -> &'static str {
        t.tag()
    }
    assert_eq!(tag_boxed(&Box::new(Vec::<u8>::new())), "boxed");

    fn tag_into<T: GlobalIntoIter>(t: &T) -> &'static str {
        t.tag()
    }
    assert_eq!(tag_into(&Vec::<u8>::new().into_iter()), "into-iter");

    fn tag_opt<T: GlobalEntry>(t: &T) -> &'static str {
        t.tag()
    }
    assert_eq!(tag_opt(&Some(0u8)), "option");
}
