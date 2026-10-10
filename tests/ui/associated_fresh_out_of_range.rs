use batch_impl::batch_impl;

#[batch_impl(Assoc<Item = @1> ().1)]
trait Assoc {
    type Item;
}

fn main() {}
