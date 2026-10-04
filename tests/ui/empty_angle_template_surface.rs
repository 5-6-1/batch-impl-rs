//! The same rule on the **`impl{...}` template** token path: `GenW<>` has no arguments to fill
//! from (the spec's trait application carries none), so it reports rather than vanishing.
//!
//! Every token surface reaches the same place (`sync_at`), which is why the surfaces share one
//! fixture family instead of one fixture per surface being "enough".
use batch_impl::batch_impl;

#[batch_impl(u8 impl{GenW<>})]
trait TemplateSurface {}

fn main() {}
