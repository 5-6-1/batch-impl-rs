use batch_impl::batch_impl;

// `self` stands for the whole right operand (`self.T` applies `T` to it); alone it
// is not a type, and used to render `impl BareSelf for self {}`.
#[batch_impl(*self)]
trait BareSelf {}

fn main() {}
