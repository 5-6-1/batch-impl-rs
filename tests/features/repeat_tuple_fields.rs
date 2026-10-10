//! Repeat cursors compose with fixed tuple fields without extra parentheses.

use batch_impl::batch_impl;

#[derive(Clone, Debug, PartialEq)]
struct Pair<A, B>(A, B);

#[batch_impl(
    (*Pair.*[self, Vec].*[].1..=3,) where { @0..: Clone }
    impl { (Column@..,) }
    #seed { @(self.@0.1.push(self.@0.0.clone());).. }
    #seed_parenthesized { @((self.@0).1.push((self.@0).0.clone());).. }
)]
trait SeedColumns {
    fn seed(&mut self);
    fn seed_parenthesized(&mut self);
}

#[test]
fn natural_field_chains_match_parenthesized_access_for_each_arity() {
    let mut one = (Pair(String::from("one"), vec![]),);
    let mut one_parenthesized = one.clone();
    one.seed();
    one_parenthesized.seed_parenthesized();
    assert_eq!(one, one_parenthesized);
    assert_eq!(one.0.1, ["one"]);

    let mut two = (Pair(7u8, vec![]), Pair(String::from("two"), vec![]));
    let mut two_parenthesized = two.clone();
    two.seed();
    two_parenthesized.seed_parenthesized();
    assert_eq!(two, two_parenthesized);
    assert_eq!(two.0.1, [7]);
    assert_eq!(two.1.1, ["two"]);

    let mut three = (Pair(true, vec![]), Pair(9u16, vec![]), Pair(String::from("three"), vec![]));
    let mut three_parenthesized = three.clone();
    three.seed();
    three_parenthesized.seed_parenthesized();
    assert_eq!(three, three_parenthesized);
    assert_eq!(three.0.1, [true]);
    assert_eq!(three.1.1, [9]);
    assert_eq!(three.2.1, ["three"]);
}

#[batch_impl(
    (((u8, (u16, u32)), (u64, (u8, u16))), (bool, (u8, u16), (u32, u64)))
    impl { ((A@..,), (bool, B@..,)) }
    #nested { (@A(self.0.@0.1,)..) }
    #deep { (@A(self.0.@0.1.1,)..) }
    #offset { (@B(self.1.@1.1,)..) }
)]
trait NestedFields {
    fn nested(&self) -> ((u16, u32), (u8, u16));
    fn deep(&self) -> (u32, u16);
    fn offset(&self) -> (u16, u64);
}

#[test]
fn fixed_fields_and_nonzero_cursors_keep_their_own_positions() {
    let value = (((1u8, (2u16, 3u32)), (4u64, (5u8, 6u16))), (false, (7u8, 8u16), (9u32, 10u64)));
    assert_eq!(value.nested(), ((2, 3), (5, 6)));
    assert_eq!(value.deep(), (3, 6));
    assert_eq!(value.offset(), (8, 10));
}

macro_rules! seed_each {
    ($($operation:expr);* $(;)?) => { $($operation;)* };
}

#[batch_impl(
    (*Pair.*[self, Vec].*[].2,) where { @0..: Clone }
    impl { @0.. }
    #seed_macro { seed_each!(@(self.@0.1.push(self.@0.0.clone());)..); }
)]
trait MacroSeed {
    fn seed_macro(&mut self);
}

trait ImplSeed {
    fn seed_fresh(&mut self);
}

#[batch_impl(T: (*Pair.*[self, Vec].*[].2,) where { @0..: Clone } impl{(Column@..,)})]
impl ImplSeed for T {
    fn seed_fresh(&mut self) {
        // The explicit driver visits each column matched by the body template.
        fresh!(seed_each!(@Column(self.@0.1.push(self.@0.0.clone());)..));
    }
}

#[test]
fn enabled_macro_inputs_and_impl_fresh_bodies_expand_field_chains() {
    let mut from_macro = (Pair(String::from("macro"), vec![]), Pair(11u32, vec![]));
    let mut from_impl = from_macro.clone();
    from_macro.seed_macro();
    from_impl.seed_fresh();
    assert_eq!(from_macro, from_impl);
    assert_eq!(from_macro.0.1, ["macro"]);
    assert_eq!(from_macro.1.1, [11]);
}
