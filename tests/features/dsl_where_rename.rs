#![allow(dead_code)]
//! Positional-substitution inheritance (the old name-equality restriction is
//! gone): renamed generics inherit bounds and where predicates — the
//! predicate's parameters are substituted positionally, path-segments
//! excluded (`A::B`'s `B` is an associated type). Each case here used to be
//! a compile-fail fixture before the upgrade.

use batch_impl::batch_impl;

// Renamed type param inherits the inline bound (`X: Clone`)
#[batch_impl(<X> A<X> ())]
trait A<T: Clone> {}

// Renamed lifetime inherits via substitution (`'a` → `'b` inside `T: 'a`)
#[batch_impl(<'b, T> A2<'b, T> ())]
trait A2<'a, T: 'a> {}

// Projection-subject predicates substitute too (`T::Item` → `U::Item`),
// path segments stay literal
trait IntoIter {
    type Item;
}

#[batch_impl(<U: IntoIter> B<U> ())]
trait B<T: IntoIter>
where
    T::Item: Clone,
{
}

impl IntoIter for std::vec::IntoIter<u8> {
    type Item = u8;
}

// Const params participate positionally (`[T; N]: Sized` → `[X; 5]: Sized`)
#[batch_impl(<X> ArrOk<X, 5> ())]
trait ArrOk<T, const N: usize>
where
    [T; N]: Sized,
{
}

// Lifetime predicates across renamed lifetimes (`'a: 'b` → `'x: 'static`)
#[batch_impl(<'x> C<'x, 'static> ())]
trait C<'a, 'b>
where
    'a: 'b,
{
}

#[test]
fn renamed_inheritance_compiles() {
    // every generated impl exists for these concrete instantiations
    fn a(_: &())
    where
        (): A<u8>,
    {
    }
    fn a2(_: &())
    where
        (): A2<'static, u8>,
    {
    }
    fn b(_: &())
    where
        (): B<std::vec::IntoIter<u8>>,
    {
    }
    fn arr(_: &())
    where
        (): ArrOk<u8, 5>,
    {
    }
    fn c(_: &())
    where
        (): C<'static, 'static>,
    {
    }

    a(&());
    a2(&());
    b(&());
    arr(&());
    c(&());
}
