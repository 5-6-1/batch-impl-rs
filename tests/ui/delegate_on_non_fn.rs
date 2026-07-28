// 错误：`#delegate` 用于 const item，应当报错"只能用于方法"
use batch_impl::batch_impl;

trait HasConst {
    const VALUE: u32;
}

#[batch_impl(usize #delegate(VALUE){0})]
trait HasConst {
    const VALUE: u32;
}

fn main() {}
