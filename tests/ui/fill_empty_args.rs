// 错误：`#fill` 参数列表为空
use batch_impl::batch_impl;

trait T {
    fn m(&self) -> u32;
}

#[batch_impl(usize #fill(){0})]
trait T {
    fn m(&self) -> u32;
}

fn main() {}
