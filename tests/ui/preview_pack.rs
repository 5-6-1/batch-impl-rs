//! Preview reports the same materialized Pack types as the real entry points.
use batch_impl::batch_preview;

batch_preview! {
    #[batch_impl((*Vec *[u8, u16],))]
    trait Vectors {}
}

batch_preview! {
    #[batch_impl((*Pair (*[self, Vec] *[].2),))]
    trait Pairs {}
}

fn main() {}
