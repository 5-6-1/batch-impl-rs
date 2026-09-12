//! Stacked `#[batch_impl]` attributes on an `impl` block: **stages of one
//! derivation**.
//!
//! rustc expands the outermost attribute first and hands it the rest; the entry
//! re-emits those attributes on the impls it derives (its `item.attrs`), so the
//! compiler expands the next stage *on those impls* — the stages therefore run in
//! **source order** over the **accumulating block**, and a slot an earlier stage
//! leaves in place is bound by a later one. The stages compose into a product.
//!
//! These tests are the contract for that order and composition: it lives in the
//! compiler's attribute expansion, so it can only be exercised through the real
//! macro (an in-process unit test would never see the next stage run).

use batch_impl::batch_impl;

struct Pair<A, B>(A, B);

trait Tag {
    fn tag(&self) -> u32;
}

// Two stages, one slot each: stage 1 binds `A`, stage 2 binds the `B` that
// stage 1 left in place. `2 × 2` impls, and the body — which mentions both
// slots — is rewritten by whichever stage binds them.
#[batch_impl(A : [u8, u16])]
#[batch_impl(B : [u32, u64])]
impl Tag for Pair<A, B> {
    fn tag(&self) -> u32 {
        (std::mem::size_of::<A>() + std::mem::size_of::<B>()) as u32
    }
}

#[test]
fn stages_bind_slots_one_after_another() {
    assert_eq!(Pair::<u8, u32>(0, 0).tag(), 5);
    assert_eq!(Pair::<u8, u64>(0, 0).tag(), 9);
    assert_eq!(Pair::<u16, u32>(0, 0).tag(), 6);
    assert_eq!(Pair::<u16, u64>(0, 0).tag(), 10);
}

// A stage can be switched off: an **empty** `#[batch_impl]` is the identity, so
// the stack below it still runs and only that stage's contribution disappears.
#[batch_impl(A : [i8, i16])]
#[batch_impl()]
#[batch_impl(B : i32)]
impl Tag for (A, B) {
    fn tag(&self) -> u32 {
        (std::mem::size_of::<A>() + std::mem::size_of::<B>()) as u32
    }
}

#[test]
fn an_empty_stage_is_the_identity() {
    assert_eq!((0i8, 0i32).tag(), 5);
    assert_eq!((0i16, 0i32).tag(), 6);
}

// A plain attribute written between two stages belongs to the **expansion level**
// it sits at: the stage above emits it on the impls it derives, and the stage
// below inherits it from them. `cfg(test)` is the always-true form here (this
// module is a test target), so it must not gate anything away — the impls below
// must exist.
#[batch_impl(C : [u8, u16])]
#[cfg(test)]
#[batch_impl(D : u32)]
impl Tag for (C, D) {
    fn tag(&self) -> u32 {
        (std::mem::size_of::<C>() + std::mem::size_of::<D>()) as u32
    }
}

#[test]
fn a_level_attribute_is_carried_onto_the_final_impls() {
    assert_eq!((0u8, 0u32).tag(), 5);
    assert_eq!((0u16, 0u32).tag(), 6);
}

// **Why the order is load-bearing.** A *shape family* — container forms that are
// not the same head (`Vec<T>`, `[T; 4]`, `Box<[T]>`, `&'static [T]`) — needs one
// prototype per family under §8.4's pattern, because a single template cannot
// match four differently shaped heads. Two stages express it directly: stage 1
// introduces the **shape with the element slot left open**, stage 2 fills that
// slot, and stage 2's substitution reaches *inside* the tokens stage 1 produced
// (the element appears in four different positions, one of them behind a
// reference). Swapping the two attributes binds the element in the untouched
// block and then introduces a `B` that nobody binds any more — measured as four
// `E0425: cannot find type `B`` (one per shape leaf) instead of eight working
// impls. This test is also the lock on the stage order: with the stages reversed
// it would not compile.
trait Elem {
    fn elem_bytes(&self) -> usize;
}

#[batch_impl(A : [Vec<B>, [B; 4], Box<[B]>, &'static [B]])]
#[batch_impl(B : [u8, u64])]
impl Elem for A {
    fn elem_bytes(&self) -> usize {
        std::mem::size_of::<B>()
    }
}

static BYTES: &[u8] = &[0, 1];

#[test]
fn a_shape_family_crossed_with_an_element_by_two_stages() {
    let boxed_u8: Box<[u8]> = Box::from([0u8; 4]);
    let boxed_u64: Box<[u64]> = Box::from([0u64; 4]);
    assert_eq!(Vec::<u8>::new().elem_bytes(), 1);
    assert_eq!(Vec::<u64>::new().elem_bytes(), 8);
    assert_eq!([0u8; 4].elem_bytes(), 1);
    assert_eq!([0u64; 4].elem_bytes(), 8);
    assert_eq!(boxed_u8.elem_bytes(), 1);
    assert_eq!(boxed_u64.elem_bytes(), 8);
    assert_eq!(BYTES.elem_bytes(), 1);
}
