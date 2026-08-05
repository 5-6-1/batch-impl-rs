// Error: inherited bound `T: 'a` references parameter 'a, but the impl does not declare a same-named lifetime
use batch_impl::batch_impl;

#[batch_impl(<'b, T> A<'b, T> ())]
trait A<'a, T: 'a> {}

fn main() {}
