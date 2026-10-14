//! Blanket wrappers preserve added predicates and prepare forwarded parameters.

use batch_impl::batch_impl;
use std::borrow::Cow;
use std::rc::Rc;

#[batch_impl(#blanket(@all_methods){@Cow where { @0: Copy }})]
trait CowTake {
    fn take(self) -> u8;
}

impl CowTake for u8 {
    fn take(self) -> u8 {
        self
    }
}

#[test]
fn cow_additional_predicates_enable_copying_the_deref_target() {
    assert_eq!(CowTake::take(Cow::Borrowed(&7u8)), 7);
    assert_eq!(CowTake::take(Cow::<u8>::Owned(9)), 9);
}

#[batch_impl(#blanket(@all_methods){@Cow where { @0: AsRef<str>, }})]
trait CowRead {
    fn bytes(&self) -> usize;
}

impl CowRead for str {
    fn bytes(&self) -> usize {
        self.len()
    }
}

#[test]
fn cow_requires_only_the_deref_target_trait_with_user_where() {
    assert_eq!(CowRead::bytes(&Cow::Borrowed("abc")), 3);
    assert_eq!(CowRead::bytes(&Cow::<str>::Owned("hello".into())), 5);
}

#[batch_impl(#blanket(@all){@Cow})]
trait CowProject<X> {
    type Output;
    const MODE: u8;
    fn project(&self, extra: X) -> Self::Output;
    fn make(value: X) -> Self::Output;
}

impl CowProject<usize> for str {
    type Output = usize;
    const MODE: u8 = 7;
    fn project(&self, extra: usize) -> usize {
        self.len() + extra
    }
    fn make(value: usize) -> usize {
        value + 1
    }
}

#[test]
fn cow_generic_trait_and_associated_items_use_the_borrowed_target() {
    assert_eq!(CowProject::project(&Cow::Borrowed("abc"), 4usize), 7);
    assert_eq!(CowProject::project(&Cow::<str>::Owned("hello".into()), 4usize), 9);
    let value: <Cow<'_, str> as CowProject<usize>>::Output = 8;
    assert_eq!(<Cow<'_, str> as CowProject<usize>>::make(value), 9);
    assert_eq!(<Cow<'_, str> as CowProject<usize>>::MODE, 7);
}

// Cow's own supertraits can require a narrower Owned predicate than Owned: Trait.
#[batch_impl(#blanket(@all_methods){@Cow where { @0::Owned: std::fmt::Debug }})]
trait CowDebug: std::fmt::Debug {
    fn size(&self) -> usize;
}

impl CowDebug for str {
    fn size(&self) -> usize {
        self.len()
    }
}

// Users can explicitly retain the old policy, including generic trait arguments.
#[batch_impl(#blanket(@all_methods){@Cow where { @0::Owned: @trait<X> }})]
trait CowOwnedPolicy<X>: std::fmt::Debug {
    fn measure(&self, extra: X) -> usize;
}

impl CowOwnedPolicy<usize> for str {
    fn measure(&self, extra: usize) -> usize {
        self.len() + extra
    }
}

impl CowOwnedPolicy<usize> for String {
    fn measure(&self, _: usize) -> usize {
        // This impl satisfies the explicit policy; forwarding must not call it.
        999
    }
}

#[test]
fn cow_supertrait_bounds_and_owned_policy_remain_explicit() {
    for value in [Cow::Borrowed("abc"), Cow::<str>::Owned("abc".into())] {
        assert_eq!(CowDebug::size(&value), 3);
        assert_eq!(CowOwnedPolicy::measure(&value, 4usize), 7);
    }
}

#[allow(non_camel_case_types)]
struct arg0;

#[batch_impl(#blanket(@all_methods){Box, Rc})]
trait BlanketNames {
    fn order(
        &self, _: u8, r#__batch_impl_arg0: u8, _: u8, __batch_impl_arg1: u8, __batch_impl_arg0_: u8,
    ) -> [u8; 6];
}

impl BlanketNames for u8 {
    fn order(&self, a: u8, b: u8, c: u8, d: u8, e: u8) -> [u8; 6] {
        [*self, a, b, c, d, e]
    }
}

#[test]
fn blanket_anonymous_names_avoid_later_raw_bindings_and_outer_arg0() {
    let _ = arg0;
    assert_eq!(BlanketNames::order(&Box::new(1), 2, 3, 4, 5, 6), [1, 2, 3, 4, 5, 6]);
    assert_eq!(BlanketNames::order(&Rc::new(6), 5, 4, 3, 2, 1), [6, 5, 4, 3, 2, 1]);
}

#[batch_impl(#blanket(@all_methods){Box})]
trait BlanketPatterns {
    // A pattern-only parameter must become an owned forwarding binding.
    #[allow(clippy::toplevel_ref_arg)]
    fn pair(&self, (first, _): (u8, u8), ref text: String) -> usize {
        usize::from(first) + text.len()
    }
    fn make<const N: usize>(_: u8) -> [u8; N];
    fn replace(&mut self, _: u8) -> u8;
}

impl BlanketPatterns for u8 {
    fn pair(&self, pair: (u8, u8), text: String) -> usize {
        usize::from(*self + pair.0 + pair.1) + text.len()
    }
    fn make<const N: usize>(value: u8) -> [u8; N] {
        [value; N]
    }
    fn replace(&mut self, value: u8) -> u8 {
        std::mem::replace(self, value)
    }
}

#[test]
fn blanket_prepares_compound_patterns_static_generics_and_mutable_methods() {
    let mut value = Box::new(2u8);
    assert_eq!(BlanketPatterns::pair(&value, (3, 5), "abcd".into()), 14);
    assert_eq!(<Box<u8> as BlanketPatterns>::make::<3>(7), [7; 3]);
    assert_eq!(BlanketPatterns::replace(&mut value, 9), 2);
    assert_eq!(*value, 9);
}
