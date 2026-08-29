//! The impl entry (0.8.0) basic tests: single-level matrix, the
//! direct form, `;`-separated multi-spec, and `unsafe impl` preservation.
//! (split from the former single-file `tests/impl_entry_impl.rs`)

use batch_impl::batch_impl;
use std::rc::Rc;

// ------------------------------------------------------------
// 1. Single-level matrix: `A<B> : [Box, Rc].[u8, u16]` → 4 impls
// ------------------------------------------------------------
#[batch_impl(A<B> : [Box, Rc].[u8, u16])]
impl Mk1 for A<B> {
    fn mk() -> A<B> {
        A::new(B::default())
    }
}

trait Mk1 {
    fn mk() -> Self;
}

#[test]
fn impl_entry_basic_matrix() {
    let b: Box<u8> = <Box<u8> as Mk1>::mk();
    assert_eq!(*b, 0);
    let r: Rc<u16> = <Rc<u16> as Mk1>::mk();
    assert_eq!(*r, 0);
    let _: Box<u16> = <Box<u16> as Mk1>::mk();
    let _: Rc<u8> = <Rc<u8> as Mk1>::mk();
}

// ------------------------------------------------------------
// 2. Direct form: `new-generic-decl? for-type` (no matrix, N = 1)
// ------------------------------------------------------------
#[batch_impl(<T> Box<T>)]
impl Mk2 for Box<T> {
    fn tag(&self) -> u32 {
        2
    }
}

trait Mk2 {
    fn tag(&self) -> u32;
}

#[test]
fn impl_entry_direct_form() {
    assert_eq!(<Box<i32> as Mk2>::tag(&Box::new(5)), 2);
    assert_eq!(<Box<u16> as Mk2>::tag(&Box::new(5)), 2);
}

// ------------------------------------------------------------
// 3. Multiple specs (`;`-separated): `W:u8; W:u16`
// ------------------------------------------------------------
#[batch_impl(W:u8; W:u16)]
impl Mk3 for W {
    fn bits() -> u32 {
        W::BITS
    }
}

trait Mk3 {
    fn bits() -> u32;
}

#[test]
fn impl_entry_multi_spec_semicolon() {
    assert_eq!(<u8 as Mk3>::bits(), 8);
    assert_eq!(<u16 as Mk3>::bits(), 16);
}

// ------------------------------------------------------------
// 7. `unsafe impl` preserved
// ------------------------------------------------------------
#[batch_impl(U:u8; U:u16)]
unsafe impl Mk7 for U {
    const TAG: u32 = U::BITS;
}

/// # Safety
///
/// Marker trait for the demo only; no real unsafe semantics.
unsafe trait Mk7 {
    const TAG: u32;
}

#[test]
fn impl_entry_unsafe_impl() {
    assert_eq!(<u8 as Mk7>::TAG, 8);
    assert_eq!(<u16 as Mk7>::TAG, 16);
}

// ------------------------------------------------------------
// 8. Partial-placeholder substitution: the shape template keeps its fixed
//    elements (`Box` matched verbatim) and only the placeholder (`T`) is
//    rewritten to the matrix leaf — `impl Tr for Box<T>` with template
//    `Box<T>` and leaf `Box<u8>` emits `impl Tr for Box<u8>`, and the
//    placeholder also rewrites inside the body (`T::BITS` → `u8::BITS`).
//    A placeholder is a substitution target, NOT a generic parameter: it
//    must not appear in the impl's own `<>` declaration (that would declare
//    a second identity for the rewritten name).
// ------------------------------------------------------------
#[batch_impl(Box<T> : [Box<u8>, Box<u16>])]
impl Mk8 for Box<T> {
    fn bits(&self) -> u32 {
        T::BITS
    }
}

trait Mk8 {
    fn bits(&self) -> u32;
}

#[test]
fn impl_entry_partial_placeholder_substitution() {
    assert_eq!(<Box<u8> as Mk8>::bits(&Box::new(0u8)), 8);
    assert_eq!(<Box<u16> as Mk8>::bits(&Box::new(0u16)), 16);
}

// ------------------------------------------------------------
// 9. The placeholder name is arbitrary (`A` here — any ident works, as long
//    as the impl's for-Type writes the same names as the template).
// ------------------------------------------------------------
#[batch_impl(Box<A> : [Box<u32>, Box<u64>])]
impl Mk9 for Box<A> {
    fn bits(&self) -> u32 {
        A::BITS
    }
}

trait Mk9 {
    fn bits(&self) -> u32;
}

#[test]
fn impl_entry_arbitrary_placeholder_name() {
    assert_eq!(<Box<u32> as Mk9>::bits(&Box::new(0u32)), 32);
    assert_eq!(<Box<u64> as Mk9>::bits(&Box::new(0u64)), 64);
}

// ------------------------------------------------------------
// 10. Placeholders in the impl's own where clause rewrite too (the where
//     predicates are rewritten by the slot mapping like the body): `A: Clone`
//     → `u8: Clone` per leaf.
// ------------------------------------------------------------
#[batch_impl(Box<A> : [Box<u8>, Box<u16>] where A: Clone)]
impl Mk10 for Box<A>
where
    A: Clone,
{
    fn bits(&self) -> u32 {
        A::BITS
    }
}

trait Mk10 {
    fn bits(&self) -> u32;
}

#[test]
fn impl_entry_placeholder_in_where_rewrites() {
    assert_eq!(<Box<u8> as Mk10>::bits(&Box::new(0u8)), 8);
    assert_eq!(<Box<u16> as Mk10>::bits(&Box::new(0u16)), 16);
}

// ------------------------------------------------------------
// 12. `@` built-in constants work on the ItemImpl entry (the matrix source
//     expands like the attribute entry): `@u*` → u8..u128, `@f*` → f32/f64,
//     `@num` → the numeric family, open/closed range families.
// ------------------------------------------------------------
#[batch_impl(W : @u*)]
impl Mk12 for W {
    fn mk() -> W {
        W::default()
    }
}

trait Mk12 {
    fn mk() -> Self;
}

#[test]
fn impl_entry_at_constants() {
    assert_eq!(<u8 as Mk12>::mk(), 0);
    assert_eq!(<u128 as Mk12>::mk(), 0);
}

#[batch_impl(W : @f*)]
impl Mk12b for W {
    fn bits(&self) -> u32 {
        32
    }
}

trait Mk12b {
    fn bits(&self) -> u32;
}

#[test]
fn impl_entry_at_f_constants() {
    assert_eq!(<f32 as Mk12b>::bits(&0.0f32), 32);
    assert_eq!(<f64 as Mk12b>::bits(&0.0f64), 32);
}

#[batch_impl(W : @num)]
impl Mk12c for W {
    fn mk() -> W {
        W::default()
    }
}

trait Mk12c {
    fn mk() -> Self;
}

#[test]
fn impl_entry_at_num_constant() {
    assert_eq!(<i8 as Mk12c>::mk(), 0);
    assert_eq!(<u32 as Mk12c>::mk(), 0);
}

// ------------------------------------------------------------
// 13. Generators + `@N..` where selectors on the ItemImpl entry: the spec
//     layer shares the attribute entry's DSL — `A<()0..=12>` mints 13 fresh
//     generics hoisted onto the impl (`impl<P0..P12> A<(P0, ..., P12)>`),
//     `where @0..: SomeTrait` constrains all of them. The body stays
//     ordinary Rust (no `@` carriers).
// ------------------------------------------------------------
struct GenA<T>(T);

trait SomeTrait {}

impl SomeTrait for u8 {}
impl SomeTrait for u16 {}

#[batch_impl(GenA<B> : GenA<()0..=3> where @0..: SomeTrait)]
impl GenA<B> {
    fn arity(&self) -> usize {
        4
    }
}

#[test]
fn impl_entry_generator_with_where_selectors() {
    // `GenA<()0..=3>` generates one impl per arity 0..=3, each with the
    // hoisted freshs constrained by `@0..: SomeTrait`.
    assert_eq!(GenA::<()>(()).arity(), 4);
    assert_eq!(GenA::<(u8,)>((0,)).arity(), 4);
    assert_eq!(GenA::<(u8, u16)>((0, 1)).arity(), 4);
    assert_eq!(GenA::<(u8, u16, u8)>((0, 1, 2)).arity(), 4);
}

// 13b. Direct form with a generator: `GenA<().3>` → hoisted freshs, no
//      matrix.
#[batch_impl(GenA<().3> where @0..: SomeTrait)]
impl GenA<B> {
    fn tag(&self) -> u32 {
        1
    }
}

#[test]
fn impl_entry_direct_form_generator() {
    let a = GenA::<(u8, u16, u8)>((0, 1, 2));
    assert_eq!(a.tag(), 1);
}

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
