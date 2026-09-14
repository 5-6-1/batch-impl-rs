// Error: the retired `-` operator is diagnosed wherever it appears, and a
// leading `.` reports the missing operand. Previously `-A` **and** a `-`
// element in a spec list (`Vec<u8>, -u16`) produced zero impls with no
// diagnostic at all — the one thing `docs/reference.md` promises never
// happens (the snapshot here used to lock that silence, because
// `TRYBUILD=overwrite` blesses a *disappeared* diagnostic as happily as a new
// one). The `-` exclusion lives only in directive argument lists.
use batch_impl::batch_impl;

#[batch_impl(-usize)]
trait DashLeft {}

#[batch_impl(Vec<u8>, -u16)]
trait DashInList {}

#[batch_impl(.isize)]
trait CaretLeft {}

fn main() {}
