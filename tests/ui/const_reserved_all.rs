use batch_impl::batch_trait;

// `@all` / `@all_*` are reserved item selectors — rejected at the constant
// definition (both the bare `@all` and the `@all_*` family).
batch_trait! {
    @all = [u8];
    A: TraitA;
}

trait TraitA {}
fn main() {}
