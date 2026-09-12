//! Associated-type bindings in **bound** positions — `<T: Iterator<Item = u8>>`,
//! `dyn Iterator<Item = u8>`, `for<'a> Iterator<Item = u8>` and their nested
//! forms. Every one of these is valid Rust, and every one used to be rejected
//! with the concrete-type diagnostic ("binding args … are only valid on a trait
//! path …"), because "is this head a trait?" was answered by the annotated
//! trait's name alone.
//!
//! The tests' value is the compile itself: the DSL's own grammar is rendered
//! back into the impl header, so a wrong acceptance would be a rustc error
//! rather than a silently dropped binding.

use batch_impl::batch_impl;

// `Box<dyn Iterator<Item = u8>>` as the target — the binding rides the trait
// object's path.
#[batch_impl(Box<dyn Iterator<Item = u8>> { fn tag(&self) -> &'static str { "boxed" } })]
trait BoxedDynIter {
    fn tag(&self) -> &'static str;
}

// A **generic declaration's** bound carries the binding (the impl inherits it
// positionally, exactly like any other written bound).
#[batch_impl(<T: Iterator<Item = u8>> IterBound<T> ())]
trait IterBound<T> {}

// An HRTB binder governs a bound, so the binding is legal under `for<'a>` too.
#[batch_impl(<T: for<'a> Iterator<Item = u8>> HrtbIterBound<T> ())]
trait HrtbIterBound<T> {}

// A `+`-joined bound list: the binding belongs to the element that carries it.
#[batch_impl(<T: Iterator<Item = u8> + Clone> IterCloneBound<T> ())]
trait IterCloneBound<T> {}

// The impl entry shares the same type parser, so its declaration carries the
// binding too (the direct form's for-type is a DSL type).
#[batch_impl(<T: Iterator<Item = u8>> Box<T>)]
impl BoundedEntry for Box<T> {
    fn tag(&self) -> &'static str {
        "entry"
    }
}

trait BoundedEntry {
    fn tag(&self) -> &'static str;
}

#[test]
fn bound_position_bindings_compile() {
    fn tag_of<T: BoxedDynIter>(t: &T) -> &'static str {
        t.tag()
    }
    let boxed: Box<dyn Iterator<Item = u8>> = Box::new(std::iter::empty());
    assert_eq!(tag_of(&boxed), "boxed");

    // Naming a type that satisfies each generated bound proves the bound (and
    // the generated impl) is well formed.
    fn check<T: Iterator<Item = u8>, W: IterBound<T> + HrtbIterBound<T> + IterCloneBound<T>>() {}
    check::<std::vec::IntoIter<u8>, ()>();

    fn tag_entry<T: BoundedEntry>(t: &T) -> &'static str {
        t.tag()
    }
    assert_eq!(tag_entry(&Box::new(Vec::<u8>::new().into_iter())), "entry");
}
