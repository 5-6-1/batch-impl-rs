use batch_impl::batch_impl;

// A trait-level where predicate whose SUBJECT is a projection (`T::Item`)
// references `T`; renaming the impl generic must error with guidance
// (previously the reference check missed projection subjects and the
// passed-through predicate made rustc report `cannot find type T` on the
// generated impl).
trait IntoIter {
    type Item;
}

#[batch_impl(<U: IntoIter> A<U> ())]
trait A<T: IntoIter>
where
    T::Item: Clone,
{
}

fn main() {}
