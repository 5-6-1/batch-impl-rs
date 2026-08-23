// A bare `where` with **no predicates** (nothing between `where` and the
// spec end) still errors — a body-less bare where is legal only when it
// carries predicates (`where T: Ord` ≡ `where T: Ord {}`).
use batch_impl::batch_impl;

#[batch_impl(<T> Sortable<T> Vec<T> where)]
trait Sortable<T> {
    fn is_sorted(&self) -> bool;
}
