//! dyn / for<'a> generator penetration: a generator inside a trait object
//! (`dyn Fn.().N`), an HRTB (`for<'a> Fn.().N`) or a nested wrapper
//! (`Box<dyn Fn.().N>`) still runs — the wrappers are structured, so the
//! fresh params escape to the impl generics and the wrapper re-renders
//! around the generated Fn type (`dyn Fn(P0,P1) + Send`).

use batch_impl::batch_impl;

// `dyn Fn.().2 + Send` as a target: the generator runs inside the dyn, the
// fresh params land on the impl, the bound tail survives
// → impl<P0,P1> DynFnGen for dyn Fn(P0,P1) + Send
#[batch_impl(dyn Fn.().2 + Send)]
trait DynFnGen {}

#[test]
fn dyn_generator_in_target() {
    fn check<T: DynFnGen + ?Sized>() {}
    check::<dyn Fn(u8, u16) + Send>();
}

// The async closure family (Rust 2024) rides the same `TyFn` structure in
// **bounds**: `AsyncFn` / `AsyncFnMut` / `AsyncFnOnce` parse as callable
// blocks. (`dyn AsyncFn…` is intentionally NOT tested: the async traits are
// not dyn-compatible on stable Rust.)
//
// → impl<F> AsyncProbe<F> for WrapP<F> where F: AsyncFn(u8)
struct WrapP<F>(F);

#[batch_impl(
    <F> AsyncProbe<F> WrapP<F> where F: AsyncFn(u8)
    { fn probe(&self) -> bool { true } }
)]
trait AsyncProbe<F> {
    fn probe(&self) -> bool;
}

#[test]
fn async_fn_bound() {
    // `AsyncFn(u8)` desugars to `Output = ()`; the closure's body reflects that
    let c = async |x: u8| {
        let _ = x;
    };
    assert!(WrapP(&c).probe());
}

// The mut/once kinds parse identically (bound position only); the compile
// check exercises the generated where predicate end-to-end.
#[batch_impl(<F> AsyncProbeM<F> WrapPM<F> where F: AsyncFnMut(u8))]
trait AsyncProbeM<F> {}

struct WrapPM<F>(F);

#[batch_impl(<F> AsyncProbeO<F> WrapPO<F> where F: AsyncFnOnce(u8))]
trait AsyncProbeO<F> {}

struct WrapPO<F>(F);

#[test]
fn async_fn_mut_and_once_bounds() {
    fn is_m<F: AsyncFnMut(u8), W: AsyncProbeM<F>>(_w: &W) -> bool {
        true
    }
    fn is_o<F: AsyncFnOnce(u8), W: AsyncProbeO<F>>(_w: &W) -> bool {
        true
    }
    let c = async |x: u8| {
        let _ = x;
    };
    assert!(is_m(&WrapPM(&c)));
    assert!(is_o(&WrapPO(c)));
}

// `dyn FnMut.().1` — the kind name renders through the dyn
#[batch_impl(dyn FnMut.().1)]
trait DynFnMutGen {}

#[test]
fn dyn_generator_fn_mut() {
    fn check<T: DynFnMutGen + ?Sized>() {}
    check::<dyn FnMut(u8)>();
}

// A generator range through the dyn: one impl per arity
// → impl<P0> DynRange for dyn Fn(P0)
// → impl<P0,P1> DynRange for dyn Fn(P0,P1)
#[batch_impl(dyn Fn.().1..3)]
trait DynRange {}

#[test]
fn dyn_generator_range() {
    fn check<T: DynRange + ?Sized>() {}
    check::<dyn Fn(u8)>();
    check::<dyn Fn(u8, u16)>();
}

// Nested wrapper: the generator sits inside a Box inside a dyn — the args of
// a target-type generic are hoisted too (`Box<dyn Fn(P0,P1)>`)
#[batch_impl(Box<dyn Fn.().2>)]
trait BoxedDyn {}

#[test]
fn boxed_dyn_generator() {
    fn check<T: BoxedDyn>() {}
    check::<Box<dyn Fn(u8, u16)>>();
}

// for<'a> through a bound (the structured HRTB form): the generator's fresh
// escapes to the impl generics, the binder stays attached
#[batch_impl(<R, T: for<'a> Fn.().1 R> HrtbBound<T, R> (@0_0,) where{@0_0: Copy} {
    fn go(&self, f: T) -> R {
        f(self.0)
    }
})]
trait HrtbBound<T, R> {
    fn go(&self, f: T) -> R;
}

#[test]
fn hrtb_bound_generator() {
    let tup = (9u8,);
    assert_eq!(HrtbBound::go(&tup, |a: u8| a * 3), 27);
}
