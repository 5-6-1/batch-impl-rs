// 错误：Raw 数字作为左侧操作数（DSL 禁止）
use batch_impl::batch_impl;

trait T {}

// `0^T` → 数字作为左侧，DSL 规则不允许
#[batch_impl(0^T)]
trait T {}

fn main() {}
