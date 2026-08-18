use batch_impl::batch_impl;

// A composite template cannot destructure a non-path target: the target
// must be structurally isomorphic to the template.
#[batch_impl(i32 impl{Rc<T>})]
trait BadShape {}

fn main() {}
