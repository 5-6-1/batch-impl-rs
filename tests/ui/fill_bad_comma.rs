// 错误：`#fill` 参数列表逗号位置不合法（连续逗号）
use batch_impl::batch_impl;

trait T {
    fn m(&self) -> u32;
    fn n(&self) -> u32;
}

#[batch_impl(usize #fill(m,,n){0})]
trait T {
    fn m(&self) -> u32;
    fn n(&self) -> u32;
}

fn main() {}
