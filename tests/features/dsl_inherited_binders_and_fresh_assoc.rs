//! Higher-ranked inheritance and associated values share the final impl context.
#![allow(dead_code)]

use batch_impl::batch_impl;

trait BorrowAt<'a> {}
trait VisitAt<'a> {}
impl<'a> BorrowAt<'a> for u8 {}
impl<'a> VisitAt<'a> for u8 {}

#[batch_impl(<U> HigherRanked<U> (U,))]
trait HigherRanked<T>
where
    for<'a> T: BorrowAt<'a> + VisitAt<'a>,
{
}

#[batch_impl(<U: Copy> HigherRankedWithInline<U> (U,))]
trait HigherRankedWithInline<T: Copy>
where
    for<'a> T: BorrowAt<'a>,
{
}

#[test]
fn inherited_binder_scopes_every_bound_in_the_predicate() {
    fn require<T: HigherRanked<u8> + HigherRankedWithInline<u8>>() {}
    require::<(u8,)>();
}

trait TwoLifetimes<'bound, 'free> {}
impl<'bound, 'free> TwoLifetimes<'bound, 'free> for &'free u8 {}

#[batch_impl(<'a, U> CaptureAvoided<'a, U> (&'a (), U))]
trait CaptureAvoided<'x, T>
where
    for<'a> T: TwoLifetimes<'a, 'x>,
{
}

#[batch_impl(<'b, U> FreeRenamed<'b, U> (&'b (), U))]
trait FreeRenamed<'x, T>
where
    for<'a> T: TwoLifetimes<'a, 'x>,
{
}

#[batch_impl(<'a, U> InlineCapture<'a, U> (&'a (), U))]
trait InlineCapture<'x, T: for<'a> TwoLifetimes<'a, 'x>> {}

#[test]
fn inherited_binders_do_not_capture_renamed_free_lifetimes() {
    fn require<'a, T>(_: &'a u8, _: std::marker::PhantomData<T>)
    where
        T: CaptureAvoided<'a, &'a u8> + FreeRenamed<'a, &'a u8> + InlineCapture<'a, &'a u8>,
    {
    }
    let value = 3;
    require(&value, std::marker::PhantomData::<(&(), &u8)>);
}

trait FunctionBound<F> {}
impl<F> FunctionBound<F> for u8 {}

#[batch_impl(<'a, U> FunctionCapture<'a, U> (&'a (), U))]
trait FunctionCapture<'x, T>
where
    T: FunctionBound<for<'a> fn(&'a u8, &'x u8)>,
{
}

macro_rules! reference {
    ($lifetime:lifetime) => {
        &$lifetime u8
    };
}
trait TypePair<A, B> {}
impl<A, B> TypePair<A, B> for u8 {}

// Opaque input is legal when the source binder needs no rename.
#[batch_impl(<'b, U> OpaqueUnchanged<'b, U> (&'b (), U))]
trait OpaqueUnchanged<'x, T>
where
    for<'a> T: TypePair<reference!('a), &'x u8>,
{
}

#[test]
fn inherited_function_pointer_binders_keep_their_scope() {
    fn require<'a, T: FunctionCapture<'a, u8> + OpaqueUnchanged<'a, u8>>(
        _: &'a (), _: std::marker::PhantomData<T>,
    ) {
    }
    require(&(), std::marker::PhantomData::<(&(), u8)>);
}

#[batch_impl(FirstFresh<Item = @0> ().1)]
trait FirstFresh {
    type Item;
}

#[batch_impl(GroupedFresh<Item = @1_0> (().1, ().1))]
trait GroupedFresh {
    type Item;
}

#[test]
fn associated_values_resolve_single_and_grouped_fresh_references() {
    let first: <(u8,) as FirstFresh>::Item = 7u8;
    let grouped: <((u8,), (u16,)) as GroupedFresh>::Item = 9u16;
    assert_eq!(first, 7);
    assert_eq!(grouped, 9);
}

#[batch_impl(AllFresh<Item = (@0..,)> ().2)]
trait AllFresh {
    type Item;
}

#[test]
fn associated_values_resolve_ranges_inside_tuple_hosts() {
    let value: <(u8, u16) as AllFresh>::Item = (3u8, 5u16);
    assert_eq!(value, (3, 5));
}

#[derive(Debug, PartialEq)]
struct P0;

#[batch_impl(FreshCollision<Item = (P0, @0)> ().1)]
trait FreshCollision {
    type Item;
}

#[test]
fn associated_values_keep_user_names_and_resolve_the_fresh_identity() {
    let value: <(u16,) as FreshCollision>::Item = (P0, 11u16);
    assert_eq!(value, (P0, 11));
}
