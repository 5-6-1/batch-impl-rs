// 错误：继承的 bound `T: 'a` 引用形参 'a，但 impl 未声明同名生命周期
use batch_impl::batch_impl;

#[batch_impl(<'b, T> A<'b, T> ())]
trait A<'a, T: 'a> {}

fn main() {}
