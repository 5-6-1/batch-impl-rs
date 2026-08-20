// Error: missing operand after `.` (trailing operator; previously the whole segment was silently dropped)
use batch_impl::batch_impl;

#[batch_impl(usize, f32 Vec. )]
trait T {
    fn tag(&self) -> &'static str { "x" }
}

fn main() {}
