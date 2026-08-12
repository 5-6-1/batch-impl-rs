use batch_impl::batch_impl;

// Array length must be a single expression; `+`/`?`/`.` cannot head a type.
#[batch_impl([u8; 3; 4])]
trait ArrT {}

#[batch_impl(.foo)]
trait PunctT {}

fn main() {}
