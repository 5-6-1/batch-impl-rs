// 错误：`-` 后缺少操作数（尾随运算符，原先会静默丢弃整段）
use batch_impl::batch_impl;

#[batch_impl(usize, f32 Vec^-)]
trait T {
    fn tag(&self) -> &'static str { "x" }
}

fn main() {}
