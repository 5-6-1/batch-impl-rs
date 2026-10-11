use batch_impl::batch_impl;

#[batch_impl(
    ((Vec<u8>,), (Vec<u16>,)) impl{((Vec<T>@..,), (Vec<U>@..,))}
    #names { vec![@(core::any::type_name::<@Vec<u8>>()),..] }
)]
trait Ambiguous {
    fn names() -> Vec<&'static str>;
}

fn main() {}
