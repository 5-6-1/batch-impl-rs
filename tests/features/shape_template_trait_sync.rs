//! `X<>` (empty trait brackets) → `X<spec args>`: where predicates,
//! `impl{...}` templates and impl-generic bounds write `Semiring<>` instead
//! of repeating `Semiring<Additive, Multiplicative>` — on **any** ident, not
//! just the spec's own trait (`impl{GenW<>}` fills like `impl{GenW<Additive,
//! Multiplicative>}`). `@trait<>` (preprocessing → trait path + `<>`) is
//! equivalent. Body sync is opt-in via a template that actually carries
//! `Tr<>` (see tests/ui/impl_trait_sync_body_negative for the negative case).

use batch_impl::batch_impl;

struct Additive;
struct Multiplicative;

// ------------------------------------------------------------
// 1. where `Semiring<>` syncs to the spec trait application — end-to-end:
//    the arity-2 impl gets `P1: Semiring<Additive, Multiplicative>` (the
//    args written once, in the spec's trait part). The switch template
//    `impl{@trait<>}` turns replacement on.
// ------------------------------------------------------------
#[batch_impl(
    Semiring<Additive, Multiplicative> ().1..=2 where{@0..: Semiring<>}
    impl{@trait<>} impl{(A@..,)} #tag1{7},
)]
trait Semiring<Oa, Om> {
    fn tag1(&self) -> u32;
}

impl Semiring<Additive, Multiplicative> for u8 {
    fn tag1(&self) -> u32 {
        7
    }
}

#[test]
fn where_trait_sync() {
    assert_eq!((7u8,).tag1(), 7);
    assert_eq!((7u8, 8u8).tag1(), 7);
}

// ------------------------------------------------------------
// 1b. `X<>` for a **non-spec** ident also fills with the spec's args — a
//     where predicate bound references the same args through empty brackets
//     (`Marker<>` → `Marker<Additive, Multiplicative>`).
// ------------------------------------------------------------
#[batch_impl(
    WrapSync<Additive, Multiplicative> (u8,) where @0..: Marker<>
    impl{@trait<>} #tag1b{1},
)]
trait WrapSync<Oa, Om> {
    fn tag1b(&self) -> u32;
}

#[allow(dead_code)]
trait Marker<Oa, Om> {}
impl Marker<Additive, Multiplicative> for u8 {}

#[test]
fn any_ident_fills() {
    assert_eq!((7u8,).tag1b(), 1);
}

// ------------------------------------------------------------
// 2. `@trait<>` is equivalent: preprocessing expands it to the trait path +
//    `<>`, then the same sync fills the brackets.
// ------------------------------------------------------------
#[batch_impl(
    @trait<Additive, Multiplicative> ().1..=2 where{@0..: @trait<>}
    impl{@trait<>} impl{(A@..,)} #tag2{7},
)]
trait AtTrait<Oa, Om> {
    fn tag2(&self) -> u32;
}

impl AtTrait<Additive, Multiplicative> for u8 {
    fn tag2(&self) -> u32 {
        7
    }
}

#[test]
fn at_trait_sync() {
    assert_eq!((7u8,).tag2(), 7);
    assert_eq!((7u8, 8u8).tag2(), 7);
}

// ------------------------------------------------------------
// 3. A trait with no generic args: `Tr<>` syncs to the bare `Tr` (brackets
//    dropped). The spec's trait is the annotated one (no trait name prefix).
// ------------------------------------------------------------
#[batch_impl(().1..=2 where{@0..: Tr<>} impl{@trait<>} impl{(A@..,)} #tag3{7})]
trait Tr {
    fn tag3(&self) -> u32;
}

impl Tr for u8 {
    fn tag3(&self) -> u32 {
        7
    }
}

#[test]
fn no_args_trait_sync() {
    assert_eq!((7u8, 8u8).tag3(), 7);
}

// ------------------------------------------------------------
// 4. Impl-generic **bound** sync: `<T: BoundSync<>>` — the empty brackets
//    are lost in the DSL parse (render drops them), so this one is synced
//    on the Ty structure (`sync_bound_ty`).
// ------------------------------------------------------------
#[batch_impl(
    <T: BoundSync<>> BoundSync<Additive, Multiplicative> Vec<T>
    impl{@trait<>} { fn n(&self) -> usize { self.len() } },
)]
trait BoundSync<Oa, Om> {
    fn n(&self) -> usize;
}

impl BoundSync<Additive, Multiplicative> for u8 {
    fn n(&self) -> usize {
        0
    }
}

#[test]
fn bound_trait_sync() {
    let v = vec![1u8, 2u8, 3u8];
    assert_eq!(v.n(), 3);
}

// ------------------------------------------------------------
// 5. Switch template `impl{BodySync<>}`: unlike ordinary shape templates it
//    does not match Self — it only syncs `BodySync<>` → `BodySync<Additive>`
//    and turns on body sync, so the body's `<Self as BodySync<>>::SIZE`
//    becomes `<Self as BodySync<Additive>>::SIZE`.
// ------------------------------------------------------------
#[batch_impl(
    BodySync<Additive> ().1..=1 where{@0..: BodySync<>} impl{BodySync<>}
    #SIZE{7}
    #tag{<Self as BodySync<>>::SIZE},
)]
trait BodySync<Oa> {
    const SIZE: u32;
    fn tag(&self) -> u32;
}

impl BodySync<Additive> for u8 {
    const SIZE: u32 = 7;
    fn tag(&self) -> u32 {
        7
    }
}

#[test]
fn body_trait_sync() {
    assert_eq!((7u8,).tag(), 7);
}
