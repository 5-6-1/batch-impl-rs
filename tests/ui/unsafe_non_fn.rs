// 错误：`unsafe` 并列非 fn 类型（忘写 `^` 的笔误；
// `unsafe` 只能修饰 fn 类型或作为裸 impl 标记 `unsafe^T`）
use batch_impl::batch_impl;

#[batch_impl(unsafe Vec<u8>)]
trait T {}

fn main() {}
