// 错误：`#delegate` 参数是解构模式，无法委托转发
use batch_impl::batch_impl_only;

#[batch_impl_only(usize #delegate(m){**self})]
trait Dummy {
    fn m(&self, (a, b): (i32, i32)) -> i32;
}

fn main() {}
