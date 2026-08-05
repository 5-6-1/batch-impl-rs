use batch_impl::batch_trait;
trait NoGen {}
batch_trait!(
    NoGen: @all_type_params;
);
fn main() {}
