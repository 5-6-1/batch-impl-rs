#![allow(dead_code)]
//! Lifetime support: `'a` is a structured leaf (`TyLifetime`), usable as a
//! generic declaration (`<'a>`), a trait argument (`Ref<'a, T>`) and inside
//! bounds (`T: 'a` / `+ 'a`). Misuse as an apply operand gets a targeted
//! diagnostic (ui: `lifetime_as_operand`).

use batch_impl::batch_impl;

// A lifetime-carrying trait batch-implemented for a wrapper family:
// `<'a, T>` declares both generics on the impl, the trait args reuse them,
// and the method borrows through the declared receiver lifetime.
struct Holder<T>(T);

#[batch_impl(
    <'a, T> Lend<'a, T> Holder<T>
    { fn lend(&'a self) -> &'a T { &self.0 } }
)]
trait Lend<'a, T> {
    fn lend(&'a self) -> &'a T;
}

#[test]
fn lifetime_declaration_and_trait_args() {
    let h = Holder(7u32);
    assert_eq!(*Lend::<u32>::lend(&h), 7);
}

// A lifetime bound on an impl generic (`T: 'a`) — the bound is a
// `+`-joined list whose element is the structured lifetime.
struct RefHolder<'a, T: ?Sized>(&'a T);

#[batch_impl(
    <'a, T: 'a> Outlives<'a, T> RefHolder<'a, T>
    { fn get(&self) -> &'a T { self.0 } }
)]
trait Outlives<'a, T> {
    fn get(&self) -> &'a T;
}

#[test]
fn lifetime_bound_in_declaration() {
    let r = RefHolder(&9i64);
    assert_eq!(*Outlives::get(&r), 9);
}

// A `where{T: 'a}` predicate form of the same constraint.
#[batch_impl(
    <'a, T> Wd<'a, T> Holder<T> where T: 'a
    { fn peek(&self) -> &'static str { "wd" } }
)]
trait Wd<'a, T> {
    fn peek(&self) -> &'static str;
}

#[test]
fn lifetime_where_predicate() {
    assert_eq!(Wd::<u8>::peek(&Holder(1u8)), "wd");
}
