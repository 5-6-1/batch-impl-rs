//! dsl.rs blanket generic-trait forms + receiver-filtered blankets:
//! multi-type/const/lifetime generic traits, `&mut` delegation, assoc
//! projections, `@all_ref_methods` filtering, static-method delegation.
//! (split from the former single-file `tests/dsl.rs`)

use batch_impl::batch_impl;

// blanket generic trait: two type params (the args of the bound `T: Two<A, B>` are grouped
// into an angle-bracket group — 0.6.1 fix: flat `<A, B>` used to be wrongly cut by the
// depth-0 comma split, only correct by render-idempotence luck; this case locks in correct
// parsing after grouping).
// Note: the `#pair` directive copies the trait signature verbatim (A/B are parameter names);
// direct impls must write concrete argument signatures by hand (no parameter substitution);
// a generic `impl<A, B> for (A, B)` would conflict with dsl_operators' PairAB `.pair()` method
// resolution, so only concrete tuples are implemented
#[batch_impl(Two<u8, u16> (u8, u16) { fn pair(&self) -> (u8, u16) { (self.0, self.1) } })]
#[batch_impl(#blanket(pair){Box})]
trait Two<A, B> {
    fn pair(&self) -> (A, B);
}

// blanket const-generic trait: `ArrWrap<4>` direct impl + `<const N: usize, T: ArrWrap<N>>`
struct Arr4;
#[batch_impl(ArrWrap<4> Arr4 { fn len(&self) -> usize { 4 } })]
#[batch_impl(#blanket(len){Box})]
trait ArrWrap<const N: usize> {
    fn len(&self) -> usize;
}

// blanket lifetime-generic trait: `impl<'a, X: Clone, T: LtWrap<'a, X>>`,
// `'a` appears only in the trait args (an unconstrained impl lifetime is legal)
#[batch_impl(LtWrap<'static, u32> u32 { fn m(&self) -> &'static str { "u32" } })]
#[batch_impl(#blanket(m){Box})]
trait LtWrap<'a, X: Clone> {
    fn m(&self) -> &'a str;
}

// blanket generic trait + `&mut self` method (Box: DerefMut delegates `(**self).inc()`)
#[batch_impl(IncGen<u16> u16 { fn inc(&mut self) -> u16 { *self += 1; *self } })]
#[batch_impl(#blanket(inc){Box})]
trait IncGen<X: Clone> {
    fn inc(&mut self) -> X;
}

// blanket non-generic trait + full assoc type/const delegation (as_trait with no args form
// `<T as Trait>::Item` / `::TAG`)
#[batch_impl(u16 {
    type Item = u32;
    const TAG: u8 = 7;
    fn tag(&self) -> u8 { 9 }
})]
#[batch_impl(#blanket(@all){Box})]
trait HasAssoc {
    type Item;
    const TAG: u8;
    fn tag(&self) -> u8;
}

#[test]
fn blanket_generic_full_forms() {
    let b: Box<(u8, u16)> = Box::new((1, 2));
    assert_eq!(b.pair(), (1u8, 2u16));
    let t = Two::<u8, u16>::pair(&(3u8, 4u16));
    assert_eq!(t, (3u8, 4u16));

    assert_eq!(Box::new(Arr4).len(), 4);
    assert_eq!(ArrWrap::<4>::len(&Arr4), 4);

    assert_eq!(Box::new(7u32).m(), "u32");

    let mut b = Box::new(5u16);
    assert_eq!(b.inc(), 6);
    assert_eq!(*b, 6);

    assert_eq!(Box::new(3u16).tag(), 9);
    assert_eq!(<Box<u16> as HasAssoc>::TAG, 7);
    let _: <Box<u16> as HasAssoc>::Item = 5u32;
}

#[test]
fn blanket_receiver_filter() {
    // `@all_ref_methods`: blanket only delegates `&self`/`&mut self` methods —
    // by-value receiver methods (delegation semantics unclear for wrappers)
    // are excluded and fall back to the trait default.
    #[batch_impl(u8 { fn by_ref(&self) -> u8 { *self } })]
    #[batch_impl(#blanket(@all_ref_methods){Box})]
    trait RecvB {
        fn by_ref(&self) -> u8;
        fn by_val(self) -> u8
        where
            Self: Sized,
        {
            0
        }
    }

    let b = Box::new(3u8);
    assert_eq!(RecvB::by_ref(&b), 3); // delegated
    assert_eq!(RecvB::by_val(b), 0); // trait default (not delegated)
}

#[batch_impl(#blanket(@all_static_methods){Box})]
trait BlanketStaticT {
    fn make() -> u8;
    fn pair(a: u8, b: u8) -> u16;
}
impl BlanketStaticT for u8 {
    fn make() -> u8 {
        7
    }
    fn pair(a: u8, b: u8) -> u16 {
        (a as u16) * 10 + b as u16
    }
}

#[test]
fn blanket_static_delegation() {
    // Static methods (no receiver) delegate through the blanket generic `t`:
    // `impl<t> BlanketStaticT for Box<t> where t: BlanketStaticT` with
    // a qualified `BlanketStaticT::make()` call — direct, chained (Box<Box<u8>>) and
    // argument-forwarding forms all reach the underlying impl.
    assert_eq!(<Box<u8> as BlanketStaticT>::make(), 7);
    assert_eq!(<Box<Box<u8>> as BlanketStaticT>::make(), 7);
    assert_eq!(<Box<u8> as BlanketStaticT>::pair(3, 4), 34);
    assert_eq!(<Box<Box<u8>> as BlanketStaticT>::pair(3, 4), 34);
}

trait OtherMethodForms {
    fn read(&self) -> u16 {
        99
    }
    fn width<T, const N: usize>() -> usize {
        99
    }
}
impl<T: ?Sized> OtherMethodForms for T {}

#[batch_impl(#blanket(@all_methods){Box})]
trait BlanketMethodForms: OtherMethodForms {
    async fn read(&self) -> u16;
    async fn add(&mut self, value: u16) -> u16;
    async fn width<T, const N: usize>() -> usize;
    fn borrowed_width<'a, T, const N: usize>(&self, value: &'a u8) -> (usize, &'a u8);
}

impl BlanketMethodForms for u16 {
    async fn read(&self) -> u16 {
        *self
    }
    async fn add(&mut self, value: u16) -> u16 {
        *self += value;
        *self
    }
    async fn width<T, const N: usize>() -> usize {
        std::mem::size_of::<T>() + N
    }
    fn borrowed_width<'a, T, const N: usize>(&self, value: &'a u8) -> (usize, &'a u8) {
        (std::mem::size_of::<T>() + N, value)
    }
}

#[test]
fn blanket_async_and_method_generics() {
    // These futures complete on their first poll; no runtime is needed. Poll
    // the generated forwarding bodies so a merely compiling stub cannot pass.
    fn ready<F: std::future::Future>(future: F) -> F::Output {
        let mut future = std::pin::pin!(future);
        let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
        match future.as_mut().poll(&mut cx) {
            std::task::Poll::Ready(value) => value,
            std::task::Poll::Pending => panic!("the test future must complete immediately"),
        }
    }

    let mut wrapped = Box::new(Box::new(7u16));
    // The supertrait deliberately has the same method names: both instance
    // and static calls must reach this trait's async, generic implementations.
    assert_eq!(OtherMethodForms::read(&wrapped), 99);
    assert_eq!(<Box<Box<u16>> as OtherMethodForms>::width::<u32, 3>(), 99);
    assert_eq!(ready(BlanketMethodForms::read(&wrapped)), 7);
    assert_eq!(ready(BlanketMethodForms::add(&mut wrapped, 5)), 12);
    assert_eq!(**wrapped, 12);
    assert_eq!(ready(<Box<Box<u16>> as BlanketMethodForms>::width::<u32, 3>()), 7);
    let value = 9;
    let (width, borrowed) = BlanketMethodForms::borrowed_width::<u64, 2>(&wrapped, &value);
    assert_eq!(width, 10);
    assert!(std::ptr::eq(borrowed, &value));
}

#[test]
fn blanket_deref_target_is_not_the_wrapper_parameter() {
    struct Redirect<T> {
        parameter: T,
        target: u16,
    }
    impl<T> std::ops::Deref for Redirect<T> {
        type Target = u16;
        fn deref(&self) -> &Self::Target {
            &self.target
        }
    }
    impl<T> std::ops::DerefMut for Redirect<T> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.target
        }
    }
    #[batch_impl(#blanket(@all){Redirect})]
    trait ReadTarget {
        fn read(&self) -> u16;
        fn add(&mut self, value: u16);
    }
    impl ReadTarget for u16 {
        fn read(&self) -> u16 {
            *self
        }
        fn add(&mut self, value: u16) {
            *self += value;
        }
    }
    impl ReadTarget for u8 {
        fn read(&self) -> u16 {
            99
        }
        fn add(&mut self, _: u16) {}
    }
    let mut redirected = Redirect { parameter: 3u8, target: 7 };
    ReadTarget::add(&mut redirected, 5);
    assert_eq!(ReadTarget::read(&redirected), 12);
    assert_eq!(ReadTarget::read(&redirected.parameter), 99);
}

#[test]
// Exercise projection constraints in both inline bounds and where predicates.
#[allow(clippy::multiple_bound_locations)]
fn blanket_self_sized_and_projected_constraints() {
    #[batch_impl(#blanket(@all){Box})]
    trait Projected {
        type Item;
        fn convert<U: From<Self::Item>>(&self, value: Self::Item) -> U
        where
            U: Into<Self::Item>;
        fn consume(self) -> u8
        where
            Self: Sized;
        fn borrow<'a>(&'a self, value: &'a Self::Item) -> &'a Self::Item
        where
            Self: 'a;
    }
    impl Projected for u8 {
        type Item = u16;
        fn convert<U: From<Self::Item>>(&self, value: Self::Item) -> U
        where
            U: Into<Self::Item>,
        {
            U::from(value)
        }
        fn consume(self) -> u8
        where
            Self: Sized,
        {
            self
        }
        fn borrow<'a>(&'a self, value: &'a Self::Item) -> &'a Self::Item
        where
            Self: 'a,
        {
            value
        }
    }
    let boxed = Box::new(7u8);
    assert_eq!(Projected::convert::<u16>(&boxed, 42), 42);
    let value = 42;
    assert!(std::ptr::eq(Projected::borrow(&boxed, &value), &value));
    assert_eq!(Projected::consume(boxed), 7);
}

#[test]
// The explicit receiver spelling is the behavior under test, including copies
// of those signatures in the generated blanket impl.
#[allow(clippy::needless_arbitrary_self_type)]
fn blanket_explicit_reference_receivers() {
    #[batch_impl(#blanket(@all){Box})]
    trait ExplicitRef {
        fn read(self: &Self) -> u16;
        fn add(self: &mut Self, value: u16);
        fn boxed(self: Box<Self>) -> u16;
    }
    impl ExplicitRef for u16 {
        fn read(self: &Self) -> u16 {
            *self
        }
        fn add(self: &mut Self, value: u16) {
            *self += value;
        }
        fn boxed(self: Box<Self>) -> u16 {
            *self
        }
    }
    let mut boxed = Box::new(7u16);
    ExplicitRef::add(&mut boxed, 5);
    assert_eq!(ExplicitRef::read(&boxed), 12);
    assert_eq!(ExplicitRef::boxed(Box::new(boxed)), 12);
}
