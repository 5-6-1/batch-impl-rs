// 错误：where 谓词继承的改名/引用检查
// 1. `T: IntoIterator` 合并进 bound 后，`<X>` 改名 → bound 改名错误
// 2. 生命周期谓词 `'a: 'b` 透传，impl 未声明 'a → 谓词引用错误
use batch_impl::batch_impl;

#[batch_impl(<X> A<X> ())]
trait A<T>
where
    T: IntoIterator,
    T::Item: Clone,
{
}

#[batch_impl(<'x> B<'x, 'static> ())]
trait B<'a, 'b>
where
    'a: 'b,
{
}

fn main() {}
