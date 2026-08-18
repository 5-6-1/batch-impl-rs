//! `X<>` (same-named empty trait brackets) → the spec trait application:
//! where predicates, `impl{...}` templates and impl-generic bounds write
//! `Semiring<>` instead of repeating `Semiring<Additive, Multiplicative>`.
//! `@trait<>` (preprocessing → trait path + `<>`) is equivalent. Body sync
//! is opt-in via a template that actually carries `Tr<>` (see
//! tests/ui/impl_trait_sync_* for the negative case). A `X<>` for any other
//! trait errors.

use batch_impl::batch_impl;

struct Additive;
struct Multiplicative;

// ------------------------------------------------------------
// 1. where `Semiring<>` syncs to the spec trait application — end-to-end:
//    the arity-2 impl gets `P1: Semiring<Additive, Multiplicative>` (the
//    args written once, in the spec's trait part).
// ------------------------------------------------------------
#[batch_impl(
    Semiring<Additive, Multiplicative> ()^1..=2 where{@0..: Semiring<>}
    impl{(A@..,)} #tag1{7},
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
// 2. `@trait<>` is equivalent: preprocessing expands it to the trait path +
//    `<>`, then the same sync fills the brackets.
// ------------------------------------------------------------
#[batch_impl(
    @trait<Additive, Multiplicative> ()^1..=2 where{@0..: @trait<>}
    impl{(A@..,)} #tag2{7},
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
#[batch_impl(()^1..=2 where{@0..: Tr<>} impl{(A@..,)} #tag3{7})]
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
    { fn n(&self) -> usize { self.len() } },
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
    BodySync<Additive> ()^1..=1 where{@0..: BodySync<>} impl{BodySync<>}
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
