//! Bound generators: `<T: Fn.().N>` — an Fn generator running inside an
//! impl-generic bound. Its fresh params must ride out of the predicate into
//! the impl generics (`impl<P0,P1, T: Fn(P0,P1)>`), never leaking a generic
//! declaration into the bound itself (which rustc rejects). The target type
//! references the same fresh group (`(@0_0, @0_1)`), so the Fn's params and
//! the target tuple's elements are the same generics.

use batch_impl::batch_impl;

// The bound generator and the target share fresh group 0: `Fn.().2` declares
// P0, P1 as the Fn's params; the target `(@0_0, @0_1)` reuses them as the
// tuple's elements. Renders as
// `impl<R, P0, P1, T: Fn(P0,P1) -> R> Apply2<T,R> for (P0,P1)`.
// The `where{... Copy}` predicates make the tuple elements copyable so the
// method can pass them by value from a `&self` receiver.
#[batch_impl(<R, T: Fn.().2 R> Apply2<T, R> (@0_0, @0_1)
    where{@0_0: Copy, @0_1: Copy} {
    fn go(&self, f: T) -> R {
        f(self.0, self.1)
    }
})]
trait Apply2<T, R> {
    fn go(&self, f: T) -> R;
}

#[test]
fn bound_generator_arity2() {
    let pair = (3u8, 7u16);
    assert_eq!(Apply2::go(&pair, |a: u8, b: u16| a + b as u8), 10);
}

// arity 0: `Fn.().0 R` declares no params — `T: Fn() -> R`; the empty target
// `()` references nothing
#[batch_impl(<R, T: Fn.().0 R> Apply0<T, R> () {
    fn go(&self, f: T) -> R {
        f()
    }
})]
trait Apply0<T, R> {
    fn go(&self, f: T) -> R;
}

#[test]
fn bound_generator_arity0() {
    assert_eq!(Apply0::go(&(), || 5u8), 5);
}

// FnMut renders its own trait name; the where predicate pins P0: Copy so the
// element can move out of the shared receiver
#[batch_impl(<R, T: FnMut.().1 R> ApplyMut<T, R> (@0_0,)
    where{@0_0: Copy} {
    fn go(&self, mut f: T) -> R {
        f(self.0)
    }
})]
trait ApplyMut<T, R> {
    fn go(&self, f: T) -> R;
}

#[test]
fn bound_generator_fn_mut() {
    let tup = (4u16,);
    assert_eq!(ApplyMut::go(&tup, |a: u16| a * 2), 8);
}

// `FnOnce.().2` — consuming receivers, no where-Copy needed: the element moves
// straight out of the owned receiver
#[batch_impl(<R, T: FnOnce.().2 R> ApplyOnce<T, R> (@0_0, @0_1) {
    fn go(self, f: T) -> R {
        f(self.0, self.1)
    }
})]
trait ApplyOnce<T, R> {
    fn go(self, f: T) -> R;
}

#[test]
fn bound_generator_fn_once() {
    let pair = (5u8, 6u16);
    assert_eq!(ApplyOnce::go(pair, |a: u8, b: u16| a as u32 + b as u32), 11);
}

// The reference example: `<R, T: Fn.().0..4 R> Tr<T> (@0..)` — one impl per
// arity 0..4 (exclusive: 0, 1, 2, 3). Each impl pins the bound to that arity
// (`T: Fn() -> R` / `T: Fn(P0) -> R` / ...) and re-opens the target range
// against that impl's own fresh list (`()` / `(P0,)` / `(P0,P1)` / ...).
// The trailing comma is optional: `(@0..)` ≡ `(@0..,)` (the arity-1 impl
// still renders a real 1-tuple `(P0,)`).
#[batch_impl(<R, T: Fn.().0..4 R> MultiArity<T, R> (@0..) {
    fn arity(&self) -> usize {
        0
    }
})]
trait MultiArity<T, R> {
    fn arity(&self) -> usize;
}

#[test]
fn bound_generator_range_multi_impl() {
    assert_eq!(<() as MultiArity<fn() -> u8, u8>>::arity(&()), 0);
    assert_eq!(<(u8,) as MultiArity<fn(u8) -> u8, u8>>::arity(&(1,)), 0);
    assert_eq!(<(u8, u16) as MultiArity<fn(u8, u16) -> u8, u8>>::arity(&(1, 2)), 0);
    assert_eq!(<(u8, u16, u32) as MultiArity<fn(u8, u16, u32) -> u8, u8>>::arity(&(1, 2, 3)), 0);
}

// The same range form with the FnMut / FnOnce kinds: each impl pins the bound
// to its own arity and re-opens the target range per impl.
#[batch_impl(<R, T: FnMut.().1..3 R> MultiMut<T, R> (@0..,) {
    fn arity(&self) -> usize {
        0
    }
})]
trait MultiMut<T, R> {
    fn arity(&self) -> usize;
}

#[test]
fn bound_generator_range_fn_mut() {
    assert_eq!(<(u8,) as MultiMut<fn(u8) -> u8, u8>>::arity(&(1,)), 0);
    assert_eq!(<(u8, u16) as MultiMut<fn(u8, u16) -> u8, u8>>::arity(&(1, 2)), 0);
}

#[batch_impl(<R, T: FnOnce.().1..3 R> MultiOnce<T, R> (@0..,) {
    fn arity(&self) -> usize {
        0
    }
})]
trait MultiOnce<T, R> {
    fn arity(&self) -> usize;
}

#[test]
fn bound_generator_range_fn_once() {
    assert_eq!(<(u8,) as MultiOnce<fn(u8) -> u8, u8>>::arity(&(1,)), 0);
    assert_eq!(<(u8, u16) as MultiOnce<fn(u8, u16) -> u8, u8>>::arity(&(1, 2)), 0);
}

// HRTB through a bound: `<T: for<'a> Fn.().1 R>` — the `for<'a>` wrapper is
// structured, so the generator inside still runs and its fresh escapes to the
// impl generics; the bound renders `T: for<'a> Fn(P0) -> R`.
#[batch_impl(<R, T: for<'a> Fn.().1 R> ApplyHrtb<T, R> (@0_0,) where{@0_0: Copy} {
    fn go(&self, f: T) -> R {
        f(self.0)
    }
})]
trait ApplyHrtb<T, R> {
    fn go(&self, f: T) -> R;
}

#[test]
fn bound_generator_hrtb() {
    let tup = (7u8,);
    assert_eq!(ApplyHrtb::go(&tup, |a: u8| a + 1), 8);
}

// The full user scenario: a bound generator driving a trait **application** —
// `Map<(@0..), Output=R>` — with a `#map` directive body. The `#map`-copied
// signature substitutes the trait's generic args verbatim (`Args` → the
// `(@0..)` tuple), whose range placeholder must re-open in the body too.
#[batch_impl(
    <R, F: Fn()2 R> Map<(@0..), Output=R> F
    #map{ self(args.0, args.1) }
)]
trait Map<Args> {
    type Output;
    fn map(&self, args: Args) -> Self::Output;
}

#[test]
fn bound_generator_with_directive_body() {
    let f = |a: u8, b: u16| a as u32 + b as u32;
    assert_eq!(Map::map(&f, (1u8, 2u16)), 3u32);
}

// The arity-adaptable body: the **fresh-binding switch** `impl{@0..}`
// declares that body modification is driven by the impl's fresh generics —
// a cursor-only repeat block (`@(…@0,)..`) repeats once per bound fresh
// (the `Fn()0..N` bound-generator arity), so one body covers every arity:
// arity 2 → `self(_args.0, _args.1)`, arity 0 → `self()`. `@{N}` names the
// N-th fresh generic (e.g. `@{0}` → the first fresh's name).
#[batch_impl(
    <R, F: Fn()0..4 R> MapAll<(@0..), Output=R> F
    impl{@0..}
    #map{ self( @(_args.@0,).. ) }
)]
trait MapAll<Args> {
    type Output;
    fn map(&self, _args: Args) -> Self::Output;
}

#[test]
fn bound_generator_fresh_driven_body() {
    let f0 = || 5u8;
    assert_eq!(MapAll::map(&f0, ()), 5);
    let f1 = |a: u8| a + 1;
    assert_eq!(MapAll::map(&f1, (4u8,)), 5);
    let f2 = |a: u8, b: u16| a as u32 + b as u32;
    assert_eq!(MapAll::map(&f2, (1u8, 2u16)), 3);
    let f3 = |a: u8, b: u16, c: u32| a as u64 + b as u64 + c as u64;
    assert_eq!(MapAll::map(&f3, (1u8, 2u16, 3u32)), 6u64);
}

// `@{N}` names a fresh generic **inside a repeat block** (it is a block-level
// marker): `@{1}` is the second fresh in document order (display name `P1`)
// — usable as a type in the generated method. The switch
// `impl{@1..=1}` binds a single fresh, so the block runs one round.
#[batch_impl(
    <R, F: Fn()2 R> TypeName<(@0..), Output=R> F
    impl{@1..=1}
    #size{ @(let _ = std::mem::size_of::<@{1}>();).. 0 }
)]
trait TypeName<Args> {
    type Output;
    fn size(&self) -> usize;
}

#[test]
fn bound_generator_fresh_name_reference() {
    let f = |a: u8, b: u16| a as u32 + b as u32;
    // the generated body must type-check `size_of::<P1>()` (P1 = u16)
    assert_eq!(TypeName::size(&f), 0);
}
