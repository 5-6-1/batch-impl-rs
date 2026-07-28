// 错误：`batch_trait!` 中 trait 路径仅由 `::` 组成，
// 没有任何识别符 → 期望标识符作为 trait 名称
use batch_impl::batch_trait;

batch_trait!(::);

fn main() {}