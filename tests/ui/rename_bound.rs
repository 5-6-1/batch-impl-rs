// 错误：impl 参数改名（X 对应形参 T），自动继承要求同名
use batch_impl::batch_impl;

#[batch_impl(<X> A<X> ())]
trait A<T: Clone> {}

fn main() {}
