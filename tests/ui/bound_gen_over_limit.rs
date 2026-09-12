use batch_impl::batch_impl;

// Three bound-generator arity ranges of 31 each: every range alone stays under
// `MAX_EXPAND` (its leaf mass is 991), but their Cartesian product is
// 31 * 31 * 31 = 29791 impls. The distribution must report the limit instead
// of rendering an illegal `T: [A, B, ...]` bound.
#[batch_impl(<T: Fn.().0..31 u8, U: Fn.().0..31 u8, V: Fn.().0..31 u8> Over<T, U, V> Marker)]
trait Over<T, U, V> {}

struct Marker;

fn main() {}
