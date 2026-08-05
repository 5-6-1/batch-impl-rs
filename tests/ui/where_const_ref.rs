// Error: composite predicate `[T; N]: Sized` references const parameter N, but the impl does not declare a same-named parameter
use batch_impl::batch_impl;

#[batch_impl(<T> ArrBad<T, 5> ())]
trait ArrBad<T, const N: usize>
where
    [T; N]: Sized,
{
}

fn main() {}
