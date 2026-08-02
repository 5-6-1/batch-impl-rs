// 错误：`#except` 排除列表为空（无意义的减法，视为笔误）
use batch_impl::batch_impl;

#[batch_impl(usize #fill(#except(#all){}){0})]
trait T {
    fn m(&self) -> u32;
}

fn main() {}
