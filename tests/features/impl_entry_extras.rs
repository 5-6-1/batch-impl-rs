//! The impl entry (0.9.0) extras: `X<>` sync in the where predicates, the
//! parallel `;`-spec reading, the attr/block generic reconciliation, slot
//! substitution in the trait path's arguments, and the lifetime namespace
//! (a `'a` is never a slot position).
//!
//! The impl block is ordinary Rust (`impl Tr<Additive, Multiplicative> for
//! ...` parses verbatim), so only the where predicates are synced — the
//! `X<>` there fills with the impl's own trait args, closing the gap with
//! the trait-entry sync (arrived in 0.9.0). Variadic segments / repeat
//! blocks are **not** supported on this entry (they are not legal Rust in
//! an impl block).

use batch_impl::batch_impl;

struct Additive;
struct Multiplicative;

// ------------------------------------------------------------
// 1. `X<>` sync in the attr where predicates: `Marker<>` fills with the
//    impl's own trait args (`impl Marked<Additive, Multiplicative> for ...`
//    → `Self: Marker<Additive, Multiplicative>`).
// ------------------------------------------------------------
trait Marker<A, B> {}
impl Marker<Additive, Multiplicative> for Box<u8> {}

trait Marked<A, B> {}

#[batch_impl(Box<u8> : Box<u8> where Self: Marker<>)]
impl Marked<Additive, Multiplicative> for Box<u8> {}

#[test]
fn impl_entry_where_sync() {
    fn check<T: Marked<Additive, Multiplicative>>() {}
    check::<Box<u8>>();
}

// ------------------------------------------------------------
// 2. `X<>` sync with a matrix: every leaf gets the synced where predicate.
// ------------------------------------------------------------
trait Marker2<A, B> {}
impl Marker2<Additive, Multiplicative> for Box<u8> {}
impl Marker2<Additive, Multiplicative> for Box<u16> {}

#[batch_impl(A<B> : Box.[u8, u16] where Self: Marker2<>)]
impl MatrixMarked<Additive, Multiplicative> for A<B> {}

trait MatrixMarked<A, B> {}

#[test]
fn impl_entry_where_sync_matrix() {
    fn check<T: MatrixMarked<Additive, Multiplicative>>() {}
    check::<Box<u8>>();
    check::<Box<u16>>();
}

// ------------------------------------------------------------
// 3. `;`-separated specs are **independent derivations from the original
//    block**, not a pipeline: each spec rewrites the body from the *original*
//    body, so the two stages stay self-consistent (`SpecW<u8>` says 1,
//    `SpecW<u16>` says 2). Under a pipeline reading the second stage would
//    receive the first stage's already-rewritten body (its `A` gone), produce
//    a `SpecW<u16>` whose body still said `u8`, and — with a multi-leaf matrix
//    per stage — emit duplicate impls (E0119) instead of compiling.
// ------------------------------------------------------------
struct SpecW<T>(T);

#[batch_impl(SpecW<A> : SpecW.u8 ; SpecW<A> : SpecW.u16)]
impl SpecTag for SpecW<A> {
    fn tag(&self) -> u32 {
        std::mem::size_of::<A>() as u32
    }
}

trait SpecTag {
    fn tag(&self) -> u32;
}

#[test]
fn impl_entry_each_spec_derives_from_the_original_block() {
    assert_eq!(SpecW::<u8>(0).tag(), 1);
    assert_eq!(SpecW::<u16>(0).tag(), 2);
}

// ------------------------------------------------------------
// 4. The attr's own generic declaration and the block's are **reconciled**: a
//    name the block already declares is not declared a second time (F4 of the
//    review pass — the natural `#[batch_impl(<T> Box<T>)] impl<T> …` spelling
//    used to hit `E0403: the name T is already used for a generic parameter`),
//    and the dropped declaration's bounds survive as a where predicate.
// ------------------------------------------------------------
#[batch_impl(<T> Box<T>)]
impl<T> DupMk for Box<T> {
    fn tag(&self) -> u8 {
        0
    }
}

trait DupMk {
    fn tag(&self) -> u8;
}

struct DupHolder<T>(T);

#[batch_impl(<T: Clone> DupHolder<T>)]
impl<T> DupClone for DupHolder<T> {
    fn tag(&self) -> u8 {
        0
    }
}

trait DupClone {
    fn tag(&self) -> u8;
}

#[test]
fn a_repeated_generic_declaration_is_reconciled() {
    assert_eq!(Box::new(0u8).tag(), 0);
    assert_eq!(DupHolder(0u8).tag(), 0);
    fn need<T: DupMk>() {}
    need::<Box<u8>>();
    fn need2<T: DupClone>() {}
    need2::<DupHolder<u8>>();
}

// ------------------------------------------------------------
// 5. A slot name written in the **trait path's arguments** is substituted like
//    every other position (F2): the parameter is stripped as a slot, so leaving
//    the name there reached the compiler unresolved (`E0425`) — the blessed
//    `impl_entry_generics` golden used to pin exactly that invalid output
//    (`impl Conv<B> for Box<u8>` with `B` gone), which is why it was re-blessed
//    as part of this fix. The path's own idents are untouched — only its angle
//    arguments are mapped.
// ------------------------------------------------------------
struct TraitArgW<T>(T);

trait LocalArg<T> {
    fn f(&self, t: &T) -> bool;
}

#[batch_impl(TraitArgW<T> : TraitArgW.u8)]
impl<T> LocalArg<T> for TraitArgW<T> {
    fn f(&self, _t: &T) -> bool {
        true
    }
}

#[test]
fn a_slot_in_the_trait_arguments_is_mapped() {
    // The generated impl is `LocalArg<u8> for TraitArgW<u8>` — the trait
    // argument followed the slot, so the call resolves.
    assert!(TraitArgW(0u8).f(&0u8));
    fn need<T: LocalArg<u8>>() {}
    need::<TraitArgW<u8>>();
}

// ------------------------------------------------------------
// 6. A **lifetime** whose name collides with a slot is not a slot position:
//    nothing ever binds a lifetime (the shape kernel compares named lifetimes
//    verbatim), so the quote-plus-ident passes through and the declaration
//    stays. Before the fix the ident was substituted inside `'a` — measured
//    `E0261: use of undeclared lifetime name `'u8`` (the declaration was also
//    dropped, because `'a` was compared against the slot set after stripping
//    its apostrophe).
// ------------------------------------------------------------
static SLOT_LIFETIME_TARGET: u8 = 7;

trait SlotLifetime<'a> {
    fn get(&self) -> &'a u8;
}

#[batch_impl(Box<a> : [Box<u8>, Box<u16>])]
impl<'a> SlotLifetime<'a> for Box<a> {
    fn get(&self) -> &'a u8 {
        &SLOT_LIFETIME_TARGET
    }
}

#[test]
fn a_lifetime_named_like_a_slot_is_left_alone() {
    // Two impls, both keeping `<'a>`: `Box<u8>` and `Box<u16>`.
    let small: Box<u8> = Box::new(3);
    assert_eq!(*small.get(), 7);
    let wide: Box<u16> = Box::new(4);
    assert_eq!(*wide.get(), 7);
    fn need<T: for<'a> SlotLifetime<'a>>() {}
    need::<Box<u8>>();
    need::<Box<u16>>();
}
