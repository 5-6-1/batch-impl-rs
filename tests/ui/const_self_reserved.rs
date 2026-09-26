use batch_impl::batch_trait;

trait Marker {}

batch_trait! {
    @Self = u8;
    Marker: @Self;
}

fn main() {}
