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
