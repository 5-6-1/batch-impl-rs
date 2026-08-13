use batch_impl::batch_impl;

// A `@name=value;` definition after the first spec is not a leading
// definition — targeted error instead of a silent passthrough.
#[batch_impl(u8, @x = [u16]; @x)]
trait BadPos {}

fn main() {}
