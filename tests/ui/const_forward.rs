use batch_impl::batch_trait;

trait T {}
batch_trait!(
    @a=@b;
    @b=[u8];
    T: @a;
);
