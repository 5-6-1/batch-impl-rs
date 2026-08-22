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

// The reference example: `<R, T: Fn.().0..4 R> Tr<T> (@0..,)` — one impl per
// arity 0..4 (exclusive: 0, 1, 2, 3). Each impl pins the bound to that arity
// (`T: Fn() -> R` / `T: Fn(P0) -> R` / ...) and re-opens the target range
// against that impl's own fresh list (`()` / `(P0,)` / `(P0,P1)` / ...).
#[batch_impl(<R, T: Fn.().0..4 R> MultiArity<T, R> (@0..,) {
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
