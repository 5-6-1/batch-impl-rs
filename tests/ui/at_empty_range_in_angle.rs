use batch_impl::batch_impl;

// An empty exclusive range **inside angle arguments**: the parse error must reach
// the entry's error channel, so the macro's own message appears (with the
// numbers) instead of a type-position `compile_error!(…);` that rustc reports as
// `expected one of `,` or `>`, found `;``. Second review round, F1 + F2.
#[batch_impl(Box<@2..1>)]
trait BadRangeInAngle {}

fn main() {}
