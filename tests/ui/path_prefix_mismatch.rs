// Error: the trailing identifier of the `#[batch_impl_only]` path prefix does not match the
// local dummy trait name.
// The last segment of path prefix `#ext::traits::Other:` is `Other`, which differs from the
// trait name `MyTrait`.
use batch_impl::batch_impl_only;

mod ext {
    pub mod traits {
        pub trait Other {}
    }
}

#[batch_impl_only(#ext::traits::Other: usize)]
trait MyTrait {}

fn main() {}
