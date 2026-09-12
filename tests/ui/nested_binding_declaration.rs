use batch_impl::batch_impl;

// An associated-type binding belongs to the **outermost** `<>` declaration, whose
// bindings become the impl's associated types. A nested declaration has no
// rendering: hoisting used to drop the binding silently (leaving an impl without
// its `type Item`), so it now reports instead. Second review round, F7c.
struct NestedHeld;

#[batch_impl((<Item = u8> NestedHeld,))]
trait NestedBinding {
    type Item;
}

fn main() {}
