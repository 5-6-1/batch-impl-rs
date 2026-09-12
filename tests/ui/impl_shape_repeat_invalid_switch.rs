use batch_impl::batch_impl;

// An `impl{...}` fresh-binding switch whose range covers no fresh
// (`@2..1` empty exclusive / `@2..=1` inverted closed) binds nothing: a
// targeted error, not a silent re-open (`@2..`) and not the misleading
// "DSL operators are not allowed" shape-template path.
#[batch_impl(().2 impl{@2..1} { fn n(&self) -> usize { 0 } })]
trait BadSwitch {
    fn n(&self) -> usize;
}

fn main() {}
