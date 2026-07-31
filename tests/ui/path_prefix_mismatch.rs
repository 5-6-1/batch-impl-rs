// 错误：`#[batch_impl_only]` 路径前缀末尾标识符与本地 dummy trait 名不一致。
// 路径前缀 `#ext::traits::Other:` 末段是 `Other`，与 trait 名 `MyTrait` 不同。
use batch_impl::batch_impl_only;

mod ext {
    pub mod traits {
        pub trait Other {}
    }
}

#[batch_impl_only(#ext::traits::Other: usize)]
trait MyTrait {}

fn main() {}
