use batch_impl::batch_impl;

// `^` was the power operator before 0.9 and `.N` replaced it. The old spelling
// must never fall through to the generic "unexpected `^` after the type" — and
// in a **bound** position it used to be dropped silently (measured: `T: Tr^u8`
// rendered `T: Tr`, the `^u8` vanished), so both routes are locked here.
#[batch_impl((u8, u16)^2)]
trait CaretTuple {}

#[batch_impl(<T: Tr^u8> CaretBound<T> u8)]
trait CaretBound<T> {}

trait Tr<A> {}

fn main() {}
