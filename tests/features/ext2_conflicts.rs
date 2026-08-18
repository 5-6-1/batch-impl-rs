//! Ext 2 (0.8.0) conflict/overlap `impl{...}` tests: "looks like a conflict
//! but is legal" cases — slots bound to composite subtrees and reused in
//! bodies, and equal-base templates with different args.
//! Compile-failing conflicts (InconsistentBinding / shape mismatches) live
//! in `tests/ui/impl_*`.

use batch_impl::batch_impl;

// ------------------------------------------------------------
// 1. Slot bound to a composite subtree: `impl{X<Y>}` matches
//    `Vec<Vec<u8>>` — X := Vec (base), Y := Vec<u8> (the whole arg subtree);
//    the body returns `X<Y>` (rewritten to the full target type)
// ------------------------------------------------------------
#[batch_impl(Vec<Vec<u8>> impl{X<Y>} { fn mk() -> X<Y> { X::new() } })]
trait CfT1 {
    fn mk() -> Self;
}

#[test]
fn impl_slot_composite_subtree() {
    let v: Vec<Vec<u8>> = <Vec<Vec<u8>> as CfT1>::mk();
    assert!(v.is_empty());
}

// ------------------------------------------------------------
// 2. Same base, different args across merged templates: `impl{Base<A>}`
//    and `impl{Base<B>}` both bind Base to Box (identical) — legal merge;
//    A and B bind the same u8 arg
// ------------------------------------------------------------
#[batch_impl(Box<u32> impl{Base<A>} impl{Base<B>} { fn mk(x: u32) -> Base<u32> { Base::new(x) } })]
trait CfT2 {
    fn mk(x: u32) -> Self;
}

#[test]
fn impl_same_base_merged() {
    assert_eq!(*<Box<u32> as CfT2>::mk(8), 8);
}

// ------------------------------------------------------------
// 3. Slot reuse across a method call and a type position in the same body:
//    `impl{W<T>}` with `W<T>` in the return type and `W::new` in the body
// ------------------------------------------------------------
#[batch_impl(Box<i16> impl{W<T>} { fn mk(x: T) -> W<T> { W::new(x) } })]
trait CfT3 {
    fn mk(x: i16) -> Self;
}

#[test]
fn impl_slot_body_and_type() {
    assert_eq!(*<Box<i16> as CfT3>::mk(1), 1);
}
