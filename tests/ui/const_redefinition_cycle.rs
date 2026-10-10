use batch_impl::batch_trait;

trait Indirect {}
batch_trait! {
    @a = u8;
    @b = @a;
    @a = @b;
    Indirect: u8;
}

trait Direct {}
batch_trait! {
    @a = u8;
    @a = @a;
    Direct: u8;
}

fn main() {}
