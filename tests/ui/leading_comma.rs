// 错误：`,,` 连续逗号与前导逗号（分隔符两侧缺少操作数）
use batch_impl::batch_impl;

#[batch_impl(,usize)]
trait LeadComma {}

#[batch_impl(usize,,isize)]
trait DoubleComma {}

fn main() {}
