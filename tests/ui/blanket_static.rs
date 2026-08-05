use batch_impl::batch_impl;

#[batch_impl(#blanket(@all_static_methods){Box})]
trait StaticT {
    fn make() -> u8;
}

fn main() {}
