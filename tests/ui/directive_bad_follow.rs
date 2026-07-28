// 错误：`#name{body}` 后既不是 `{body}` 也不是 `(args){body}`
use batch_impl::batch_impl;

trait T {
    fn m(&self) -> u32;
}

#[batch_impl(usize #m[not_a_group_or_parens])]
trait T {
    fn m(&self) -> u32;
}

fn main() {}
