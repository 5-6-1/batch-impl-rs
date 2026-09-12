//! The impl entry (0.8.0), composed specs: the `fresh!` body marker, textual
//! substitution, the second shape template (`impl{...}`), the block model,
//! `where{...}` at any position, multi-template merging, and slot-named item
//! generics.
//! (split from `tests/features/impl_entry_basic.rs`)

use batch_impl::batch_impl;
use std::rc::Rc;

// The generator carrier — the same fixture as `impl_entry_basic` (each
// per-feature module stays self-contained).
struct GenA<T>(T);

// ------------------------------------------------------------
// 14. `fresh!` — the body-level DSL marker (the attribute entry's repeat
//     protocol wrapped in a legal macro-call spelling): `@ident` is an
//     implicit segment bound to this impl's fresh generics (`(@(@T,)..)` →
//     `(P0, P1, P2, P3)`), `@{N}` names the N-th fresh. The marker is fully
//     expanded — the output never contains a `fresh!` call.
// ------------------------------------------------------------
trait TupleTr {
    type MyTuple;
}

#[batch_impl(GenA<B> : GenA<()1..=3>)]
impl TupleTr for GenA<B> {
    type MyTuple = (fresh!(@(@T,)..));
}

#[test]
fn impl_entry_fresh_marker_segment_repeat() {
    type T3 = <GenA<(u8, u16, u8)> as TupleTr>::MyTuple;
    let _: T3 = (0u8, 1u16, 2u8);
}

trait FirstTr {
    type First;
}

#[batch_impl(GenA<B> : GenA<()1..=2>)]
impl FirstTr for GenA<B> {
    type First = fresh!(@{0});
}

#[test]
fn impl_entry_fresh_marker_direct_ref() {
    type F = <GenA<(u8, u16)> as FirstTr>::First;
    let _: F = 7u8;
}

// ------------------------------------------------------------
// 15. Textual substitution (the non-matching mode): the template `Box<T>`
//     matches each matrix leaf and the slot mapping is applied to the impl's
//     for-Type **verbatim** — the for-Type need not mirror the template
//     (`Vec<T>` here): the slot `T` still rewrites (`Vec<u8>` / `Vec<u16>`).
// ------------------------------------------------------------
#[batch_impl(Box<T> : [Box<u8>, Box<u16>])]
impl TextTr for Vec<T> {
    fn tag(&self) -> u32 {
        1
    }
}

trait TextTr {
    fn tag(&self) -> u32;
}

#[test]
fn impl_entry_textual_substitution() {
    assert_eq!(<Vec<u8> as TextTr>::tag(&vec![0u8]), 1);
    assert_eq!(<Vec<u16> as TextTr>::tag(&vec![0u16]), 1);
}

// ------------------------------------------------------------
// 16. The second shape template (`impl{...}` — the attr entry's spelling):
//     `A<B> : [Box,Rc].().2..=3 impl A<(T@..)>` — one matrix source
//     (2 containers × 2 arities, no Cartesian combination); template 1
//     (`A<B>`) drives the for-Type, template 2 (`A<(T@..)>`) declares the
//     `T@..` segment the body's `fresh!` references.
// ------------------------------------------------------------
trait TupleTr2 {
    type MyTuple;
}

#[batch_impl(A<B> : [Box,Rc].().2..=3 impl A<(T@..)>)]
impl TupleTr2 for A<B> {
    type MyTuple = (fresh!(@(@T,)..));
}

#[test]
fn impl_entry_second_template_segment() {
    type M2 = <Box<(u8, u16)> as TupleTr2>::MyTuple;
    let _: M2 = (0u8, 1u16);
    type M3 = <Rc<(u8, u16, u8)> as TupleTr2>::MyTuple;
    let _: M3 = (0u8, 1u16, 2u8);
    type M4 = <Rc<(u8, u16)> as TupleTr2>::MyTuple;
    let _: M4 = (0u8, 1u16);
}

// ------------------------------------------------------------
// 17. The block model: each matrix element pairs a container with its own
//     `impl{...}` template at any position (`[[Box,Rc]impl{A<(T@..)>},
//     Vec impl{Vec<(T@..)>}].().2..=3`) — one matrix source, each leaf
//     matched by its own template (the `T@..` segment drives `fresh!`).
// ------------------------------------------------------------
trait TupleTr3 {
    type MyTuple;
}

#[batch_impl(A<B> : [[Box,Rc]impl{A<(T@..)>}, Vec impl{Vec<(T@..)>}].().2..=3)]
impl TupleTr3 for A<B> {
    type MyTuple = (fresh!(@(@T,)..));
}

#[test]
fn impl_entry_block_model_per_container_templates() {
    type M = <Box<(u8, u16)> as TupleTr3>::MyTuple;
    let _: M = (0u8, 1u16);
    type M2 = <Vec<(u8, u16, u8)> as TupleTr3>::MyTuple;
    let _: M2 = (0u8, 1u16, 2u8);
}

// ------------------------------------------------------------
// 18. `where{...}` composes at **any position** (the block model — a
//     `WithWhere` attachment like every other block): predicates extracted
//     from the middle / before the colon apply with the slot substitution
//     (`B: MyTrait` → `u8: MyTrait` per leaf).
// ------------------------------------------------------------
trait WhTr {
    fn tag(&self) -> u32;
}

trait MyTrait {}
impl MyTrait for u8 {}

// where between the template and the matrix (not trailing)
#[batch_impl(A<B> where{B: MyTrait} : [Box,Rc].u8)]
impl WhTr for A<B> {
    fn tag(&self) -> u32 {
        1
    }
}

#[test]
fn impl_entry_where_any_position() {
    assert_eq!(<Box<u8> as WhTr>::tag(&Box::new(0u8)), 1);
    assert_eq!(<Rc<u8> as WhTr>::tag(&Rc::new(0u8)), 1);
}

// multiple where attachments in one spec, comma-joined
trait WhTr2 {
    fn tag(&self) -> u32;
}

#[batch_impl(A<B> where{B: Clone} : [Box,Rc].u8 where{B: Default})]
impl WhTr2 for A<B> {
    fn tag(&self) -> u32 {
        2
    }
}

#[test]
fn impl_entry_where_multiple_attachments() {
    assert_eq!(<Box<u8> as WhTr2>::tag(&Box::new(0u8)), 2);
    assert_eq!(<Rc<u8> as WhTr2>::tag(&Rc::new(0u8)), 2);
}

// ------------------------------------------------------------
// Multi-template merge: two shape templates on one matrix leaf — the
// impl entry must keep and merge BOTH (the same semantics as the attribute
// entry's `collect_shape_mapping` over multiple templates; the impl entry
// used to keep only the last, silently dropping the first).
// The matrix `[Box, Rc] u8` produces leaves `Box<u8>` / `Rc<u8>`;
// `A<B>` binds A := Box, B := u8; `C<D>` binds C := Box, D := u8.
// The body uses both A and C — either dropped template leaves a slot
// unbound and the body fails to compile.
// ------------------------------------------------------------
#[batch_impl(A<B> : [Box, Rc] u8 impl{A<B>} impl{C<D>})]
impl MkMulti for A<B> {
    fn mk() -> A<B> {
        // A and B (return type) come from the first template, C and D from
        // the second; using C proves the second template was not dropped,
        // returning A<B> proves the first survived the merge.
        C::new(1)
    }
}

trait MkMulti {
    fn mk() -> Self;
}

#[test]
fn impl_entry_multi_template_merge() {
    let b: Box<u8> = <Box<u8> as MkMulti>::mk();
    assert_eq!(*b, 1);
    let r: Rc<u8> = <Rc<u8> as MkMulti>::mk();
    assert_eq!(*r, 1);
}

// ------------------------------------------------------------
// Slot-named generic on the impl block is a substitution target, not a
// declaration: `impl<T> Mk for Wrapper<T>` with template slot `T` (bound to
// `u8` by the leaf) used to emit `impl<T> Mk for Box<u8>` — rustc E0207
// (unconstrained `T`). The redundant `<T>` is now stripped; the body's `T`
// still substitutes (`Wrapper::new(T::default())` → `Box::new(u8::default())`).
// ------------------------------------------------------------
#[batch_impl(Wrapper<T> : [Box, Rc].u8)]
impl<T> MkP2 for Wrapper<T> {
    fn make() -> Wrapper<T> {
        Wrapper::new(T::default())
    }
}

trait MkP2 {
    fn make() -> Self;
}

#[test]
fn impl_entry_slot_named_item_generic_stripped() {
    let b: Box<u8> = <Box<u8> as MkP2>::make();
    assert_eq!(*b, 0);
    let r: Rc<u8> = <Rc<u8> as MkP2>::make();
    assert_eq!(*r, 0);
}

// The stripped param's bounds carry over as a where predicate on the
// substituted type: `impl<T: Clone>` → `where u8: Clone` (not silently
// dropped with the param).
#[batch_impl(Wrapper<T> : [Box, Rc].u8)]
impl<T: Clone> MkP2b for Wrapper<T> {
    fn make() -> Wrapper<T> {
        Wrapper::new(T::default())
    }
}

trait MkP2b {
    fn make() -> Self;
}

#[test]
fn impl_entry_slot_named_param_bounds_become_where() {
    let b: Box<u8> = <Box<u8> as MkP2b>::make();
    assert_eq!(*b, 0);
    let r: Rc<u8> = <Rc<u8> as MkP2b>::make();
    assert_eq!(*r, 0);
}
