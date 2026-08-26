use batch_impl::batch_impl;

// a splat cannot be an associated-type binding value — bindings take exactly
// one type (distribute via a spec list like `[Tr<Item=A>, Tr<Item=B>]`)
#[batch_impl(BadBindingSplat<Item=*(A,B)> usize)]
trait BadBindingSplat {
    type Item;
}

fn main() {}
