// 错误：范围为空（起始不小于结束），不会生成任何 impl
use batch_impl::batch_impl;

#[batch_impl(()^3..2)]
trait EmptyRange {}

fn main() {}
