// 错误：`^N` 展开产物数量超过上限 1024，视为笔误
use batch_impl::batch_impl;

#[batch_impl(()^2000)]
trait TooMany {}

fn main() {}
