// Error: missing operand before `.`/`-` (empty left side. Previously `-A` silently swallowed
// a segment and `.A` generated garbage types)
use batch_impl::batch_impl;

#[batch_impl(-usize)]
trait DashLeft {}

#[batch_impl(.isize)]
trait CaretLeft {}

fn main() {}
