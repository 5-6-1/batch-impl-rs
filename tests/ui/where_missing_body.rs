// A bare `where predicate` in the new syntax missing a code block should error in preprocessing
use batch_impl::batch_impl;

#[batch_impl(<T> Sortable<T> Vec<T> where T: Ord)]
trait Sortable<T> {
    fn is_sorted(&self) -> bool;
}
