// Error: `#delegate` argument is a destructuring pattern, cannot be forwarded
use batch_impl::batch_impl_only;

#[batch_impl_only(usize #delegate(m){**self})]
trait Dummy {
    fn m(&self, (a, b): (i32, i32)) -> i32;
}

fn main() {}
