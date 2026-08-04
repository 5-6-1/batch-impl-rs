use batch_impl::batch_trait;

trait T {}
batch_trait!(
    @a=@a;
    T: @a;
);
