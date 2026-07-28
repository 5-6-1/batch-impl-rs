// 错误：`#name{body}` 引用的 item 不在 trait 中
use batch_impl::batch_impl;

trait T {
    fn m(&self) -> u32;
}

#[batch_impl(usize #no_such{0})]
trait T {
    fn m(&self) -> u32;
}

fn main() {}
