// 错误：排除全部（`-@all`）后保留列表为空
use batch_impl::batch_impl;

#[batch_impl(usize #fill(@all,-@all){0})]
trait T {
    fn m(&self) -> u32;
}

fn main() {}
