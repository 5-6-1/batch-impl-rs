use batch_impl::batch_impl;

// Custom `@name=value;` constant sections are `batch_trait!`-only — an
// attribute-macro definition errors (the 0.7.2 feature was reverted in
// 0.8.0; write the matrix with `.` / `-` / `*` instead).
#[batch_impl(@small = [u8, u16]; @small)]
trait BadAttr {}

fn main() {}
