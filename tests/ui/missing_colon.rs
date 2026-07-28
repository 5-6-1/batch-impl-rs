// 错误：`batch_trait!(A)` → 期望 ':' 分隔 trait 名称和 impl-specs
use batch_impl::batch_trait;

trait A {}

// 没有冒号紧跟在 trait 名后
batch_trait!(A);

fn main() {}
