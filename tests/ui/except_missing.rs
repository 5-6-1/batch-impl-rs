// 错误：`#except` 缺少排除列表（需两个括号参数）
use batch_impl::batch_impl;

#[batch_impl(usize #fill(#except(#all)){0})]
trait T {
    fn m(&self) -> u32;
}

fn main() {}
