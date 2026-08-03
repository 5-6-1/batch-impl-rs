// 错误：复合谓词 `[T; N]: Sized` 引用 const 形参 N，impl 未声明同名参数
use batch_impl::batch_impl;

#[batch_impl(<T> ArrBad<T, 5> ())]
trait ArrBad<T, const N: usize>
where
    [T; N]: Sized,
{
}

fn main() {}
