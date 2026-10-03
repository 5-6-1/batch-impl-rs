use batch_impl::batch_impl;

// A `where{…}` block constrains a type. With nothing to attach to it used to render
// `impl BareWhere for where u8: Clone {}`.
#[batch_impl(*where { u8: Clone })]
trait BareWhere {}

fn main() {}
