use batch_impl::batch_impl;

// A `+` in a `dyn` bound list needs a bound after it. The old code emitted the
// bare `+` token and let rustc report "expected trait, found `+`" against the
// macro input; the DSL now says what is missing and points at the `+`.
#[batch_impl(dyn Send +)]
trait DanglingDynBound {}

fn main() {}
