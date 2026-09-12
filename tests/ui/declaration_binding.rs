use batch_impl::batch_impl;

// An associated-type binding belongs on the **trait application**
// (`AssocOnTrait<Item = u8> Target`), not in a generic-declaration block: a
// `<>` block declares parameters, and a binding there declares nothing. It used
// to be honoured as the impl's associated type at the outermost position and
// dropped silently when nested — an unintended spelling either way, so both
// positions report the same diagnostic and name the spelling that works.
struct RootHeld;

#[batch_impl(<Item = u8> RootHeld)]
trait RootBindingDecl {
    type Item;
}

// The nested position (inside a tuple) is the one that used to be dropped
// without a diagnostic.
struct NestedHeld;

#[batch_impl((<Item = u8> NestedHeld,))]
trait NestedBindingDecl {
    type Item;
}

fn main() {}
