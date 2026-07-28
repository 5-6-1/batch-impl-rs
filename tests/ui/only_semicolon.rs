// 错误：`batch_trait!(unsafe)` — `unsafe` 后没有任何 token，
// macro 期望 trait 名称
use batch_impl::batch_trait;

batch_trait!(unsafe);

fn main() {}