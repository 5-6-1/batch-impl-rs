// An empty attribute is a no-op on the impl entry: nothing is derived, so the
// original block is emitted unchanged. Compile-only (trybuild's pass path), but
// the `need` calls below fail if the impl was swallowed.
use batch_impl::batch_impl;

trait EmptyAttr {
    fn tag(&self) -> u32;
}

#[batch_impl]
impl EmptyAttr for u8 {
    fn tag(&self) -> u32 {
        1
    }
}

#[batch_impl()]
impl EmptyAttr for u16 {
    fn tag(&self) -> u32 {
        2
    }
}

fn main() {
    fn need<T: EmptyAttr>() {}
    need::<u8>();
    need::<u16>();
}
