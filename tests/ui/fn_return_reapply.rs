use batch_impl::batch_impl;

// `fn(A) -> B C`: the return type is a **type position**, so the space
// application folds as everywhere else — `C` becomes `B`'s argument
// (`fn(u8) -> u16 u32` renders `fn(u8) -> u16<u32>`, rustc's E0109). The
// snapshot locks rustc's own words: with symbolic names it is E0425 per name.
// (The old comment promised DSL "guidance" here; measured, there is none —
// `C` is applied, not dropped, and `-> Box u8` = `Box<u8>` depends on this
// fold, so no rejection is possible.)
#[batch_impl(fn(A) -> B   C)]
trait FnReturnReapply {}

fn main() {}
