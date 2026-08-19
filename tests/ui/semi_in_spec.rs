use batch_impl::batch_impl;

// `;` is the batch_trait! segment boundary; inside #[batch_impl] it is not
// part of any spec and must error with guidance instead of leaking into the
// generated impl (previously `A.B; C` rendered `impl ... for A<B; C>` and
// rustc reported "expected one of ..., found `;`" with no batch-impl hint).
#[batch_impl(A.B; C)]
trait SemiBoundary {}

fn main() {}
