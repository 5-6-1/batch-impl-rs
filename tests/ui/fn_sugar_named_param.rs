use batch_impl::batch_impl;

// A named parameter is valid Rust only in a `fn(...)` **pointer** type:
// `Fn(x: u8)` is "`Trait(...)` syntax does not support named parameters", so
// the sugar gets its own message instead of a bare "unexpected `:` after the
// type".
#[batch_impl(Box<dyn Fn(x: u8) -> u8>)]
trait BadSugar {}

fn main() {}
