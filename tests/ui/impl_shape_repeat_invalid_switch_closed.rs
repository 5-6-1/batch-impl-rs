use batch_impl::batch_impl;

// The sibling of `impl_shape_repeat_invalid_switch.rs`, for the **closed**
// inverted form: `@2..=1` is normalized through the inclusive protocol, and the
// inverted range binds no fresh either — the same targeted error, from the other
// detection branch (an exclusive range is caught while normalizing `..M` to
// `..=M-1`; a closed one only after the range is built).
#[batch_impl(().2 impl{@2..=1} { fn n(&self) -> usize { 0 } })]
trait BadClosedSwitch {
    fn n(&self) -> usize;
}

fn main() {}
