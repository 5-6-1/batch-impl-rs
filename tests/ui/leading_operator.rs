// 错误：`^`/`-` 前缺少操作数（左空。原先 `-A` 静默吞段、`^A` 生成垃圾类型）
use batch_impl::batch_impl;

#[batch_impl(-usize)]
trait DashLeft {}

#[batch_impl(^isize)]
trait CaretLeft {}

fn main() {}
