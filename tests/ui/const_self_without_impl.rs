use batch_impl::{batch_impl, batch_impl_only, batch_trait};

#[batch_impl(@Self)]
trait Attribute {}

#[batch_impl_only(@Self)]
trait Only {}

trait FunctionLike {}
batch_trait! {
    FunctionLike: @Self;
}

batch_trait! {
    @source = @Self;
    FunctionLike: @source;
}

fn main() {}
