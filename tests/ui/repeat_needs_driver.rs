// A body repeat block `@(…)..` must know its length: either a variadic segment
// drives it (`impl{(A@..,)}`) or the fresh-binding switch does (`impl{@0..}`).
// Without both, the macro reports it — a cursor-only block with a template but
// no switch has its own message (`impl_shape_repeat_cursor_multi`).
use batch_impl::batch_impl;

#[batch_impl(u8 { fn n(&self) -> usize { @(A,).. } })]
trait RepeatNeedsDriver {}

fn main() {}
