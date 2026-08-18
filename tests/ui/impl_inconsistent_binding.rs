use batch_impl::batch_impl;

// The same slot bound to different subtrees across merged `impl{...}`
// templates: `impl{X}` binds the whole leaf (`Box<u32>`), `impl{X<u32>}`
// binds the base (`Box`) — InconsistentBinding, no override.
#[batch_impl(Box<u32> impl{X} impl{X<u32>})]
trait BadMerge {}

fn main() {}
