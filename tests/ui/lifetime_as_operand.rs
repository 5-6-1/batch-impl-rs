use batch_impl::batch_impl;

// a lifetime is not an apply operand — it belongs in bounds, declarations
// or references (`&'a T`), all of which parse it as a leaf
#[batch_impl(<T> Bad<'a T> Holder<T>)]
trait BadLifetimeOperand {
    fn x(&self);
}

struct Holder<T>(T);

fn main() {}
