// Error: `batch_trait!(unsafe)` — nothing follows `unsafe`,
// the macro expects a trait name
use batch_impl::batch_trait;

batch_trait!(unsafe);

fn main() {}