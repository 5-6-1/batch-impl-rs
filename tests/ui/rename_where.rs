// Error: renaming/reference checks for inherited where predicates
// 1. `T: IntoIterator` merges into a bound; renaming to `<X>` → bound renaming error
// 2. lifetime predicate `'a: 'b` passes through; the impl does not declare 'a → predicate reference error
use batch_impl::batch_impl;

#[batch_impl(<X> A<X> ())]
trait A<T>
where
    T: IntoIterator,
    T::Item: Clone,
{
}

#[batch_impl(<'x> B<'x, 'static> ())]
trait B<'a, 'b>
where
    'a: 'b,
{
}

fn main() {}
