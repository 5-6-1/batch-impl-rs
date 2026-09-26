use batch_impl::batch_impl;

struct TypeBox<T>(T);
struct ConstBox<const N: usize>([u8; N]);
trait Mark {}

// Both bare arguments parse as syn types; the declaration still makes N const.
#[batch_impl(TypeBox<T>: <const N: usize> ConstBox<N>)]
impl<T> Mark for TypeBox<T> {}

fn main() {}
