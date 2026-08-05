// 错误：`-` 后缺排除目标（期望标识符或 `@all` 标记）
use batch_impl::batch_impl;

#[batch_impl(usize #fill(a, -){0})]
trait T {
    fn a(&self) -> u32;
}

fn main() {}
