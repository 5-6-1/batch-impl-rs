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
    // `fn make() -> u8 { t::make() }` — direct, chained (Box<Box<u8>>) and
    // argument-forwarding forms all reach the underlying impl.
    assert_eq!(<Box<u8> as BlanketStaticT>::make(), 7);
    assert_eq!(<Box<Box<u8>> as BlanketStaticT>::make(), 7);
    assert_eq!(<Box<u8> as BlanketStaticT>::pair(3, 4), 34);
    assert_eq!(<Box<Box<u8>> as BlanketStaticT>::pair(3, 4), 34);
}
