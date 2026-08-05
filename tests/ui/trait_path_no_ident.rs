// Error: the trait path in `batch_trait!` consists only of `::`,
// with no identifier → expected an identifier as the trait name
use batch_impl::batch_trait;

batch_trait!(::);

fn main() {}