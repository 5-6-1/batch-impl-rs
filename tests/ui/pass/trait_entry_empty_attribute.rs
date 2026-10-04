// An empty attribute is a no-op on the trait entry too: nothing is derived, so the trait is
// emitted unchanged. `#[batch_impl(;)]` counts as empty because separators are not content —
// `docs/reference.md` documents both spellings as "re-emit the item unchanged". Compile-only
// (trybuild's pass path), but the `need` calls below fail if a trait was swallowed.
use batch_impl::batch_impl;

#[batch_impl(;)]
trait SemicolonOnly {}

#[batch_impl()]
trait EmptyAttribute {}

impl SemicolonOnly for u8 {}

impl EmptyAttribute for u16 {}

fn main() {
    fn need<T: SemicolonOnly>() {}
    fn need2<T: EmptyAttribute>() {}
    need::<u8>();
    need2::<u16>();
}
