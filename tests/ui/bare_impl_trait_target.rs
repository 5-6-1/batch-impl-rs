use batch_impl::batch_impl;

// The pre-0.9.5 `impl Trait` target spelling is not a shape template — a
// bare `impl <trait-object>` in the spec must report a targeted diagnostic
// instead of being collected into a template that silently renders an empty
// target type. Write `dyn Fn() -> u8` for the trait-object type or an
// `impl{...}` template.
#[batch_impl(impl Fn() -> u8 { fn f() -> u8 { 0 } })]
trait BareImplTraitTarget {
    fn f() -> u8;
}

fn main() {}
