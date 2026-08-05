// Error: `batch_trait!(A)` → expected ':' separating the trait name and impl-specs
use batch_impl::batch_trait;

trait A {}

// no colon immediately after the trait name
batch_trait!(A);

fn main() {}
