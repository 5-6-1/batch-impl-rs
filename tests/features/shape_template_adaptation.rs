//! Shape adaptation preserves matrix leaves and declared parameter kinds.

use batch_impl::batch_impl;

#[batch_impl((u16, u32) impl{(u8, u16)} #value{(u8::MAX, u16::MAX)})]
trait MatrixLeaf {
    fn value() -> Self;
}

trait ImplPrototype {
    fn value() -> Self;
}

#[batch_impl((u8, u16): (u16, u32))]
impl ImplPrototype for (u8, u16) {
    fn value() -> Self {
        (u8::MAX, u16::MAX)
    }
}

#[test]
fn matrix_leaf_and_once_mapped_prototype_agree() {
    assert_eq!(<(u16, u32) as MatrixLeaf>::value(), (u16::MAX, u32::MAX));
    assert_eq!(<(u16, u32) as ImplPrototype>::value(), (u16::MAX, u32::MAX));
}

struct Wrap<const N: usize>([u8; N]);
trait Width {
    fn width(&self) -> usize;
}

#[batch_impl(Wrap<N>: [Wrap<2>, Wrap<3>])]
impl<const N: usize> Width for Wrap<N> {
    fn width(&self) -> usize {
        N
    }
}

trait AttrWidth {
    fn width(&self) -> usize;
}

#[batch_impl(Wrap<N>: <const N: usize> [Wrap<4>, Wrap<5>])]
impl AttrWidth for Wrap<N> {
    fn width(&self) -> usize {
        N
    }
}

#[batch_impl(
    <const N: usize> ConstMembers<N, Item = [u8; N]> [Wrap<6>, Wrap<7>]
        impl{Wrap<N>} #width{N}
)]
trait ConstMembers<const LENGTH: usize> {
    type Item;
    fn width(&self) -> usize;
}

trait SizedFor<const N: usize> {}
impl SizedFor<2> for u8 {}
struct PairWrap<T, const N: usize>(T, [u8; N]);

#[batch_impl(
    <const N: usize, T: SizedFor<N>> ConstDependent<N> PairWrap<T, 2>
        impl{PairWrap<T, N>}
)]
trait ConstDependent<const N: usize> {}

// The uppercase label deliberately shares the const slot's spelling.
#[batch_impl(
    <const N: usize> ConstFunctionLabel<fn(N: [u8; N])>
        #[allow(non_snake_case)] Wrap<8> impl{Wrap<N>}
)]
trait ConstFunctionLabel<F> {}

#[test]
fn const_arguments_bind_from_item_or_attribute_declarations() {
    assert_eq!(Width::width(&Wrap([0; 2])), 2);
    assert_eq!(Width::width(&Wrap([0; 3])), 3);
    assert_eq!(AttrWidth::width(&Wrap([0; 4])), 4);
    assert_eq!(AttrWidth::width(&Wrap([0; 5])), 5);
    assert_eq!(<Wrap<6> as ConstMembers<6>>::width(&Wrap([0; 6])), 6);
    assert_eq!(<Wrap<7> as ConstMembers<7>>::width(&Wrap([0; 7])), 7);
    let _: <Wrap<6> as ConstMembers<6>>::Item = [0u8; 6];
    let _: <Wrap<7> as ConstMembers<7>>::Item = [0u8; 7];
    fn dependent<T: ConstDependent<2>>() {}
    dependent::<PairWrap<u8, 2>>();
    fn const_label<T: ConstFunctionLabel<fn([u8; 8])>>() {}
    const_label::<Wrap<8>>();
}

trait Identity {}
#[batch_impl(Wrap<N>: Wrap<N>)]
impl<const N: usize> Identity for Wrap<N> {}

#[test]
fn an_unchanged_const_parameter_stays_declared() {
    fn check<T: Identity>() {}
    check::<Wrap<17>>();
}

trait Invoke {
    type Input;
    type Output;
    fn invoke(&self, value: Self::Input) -> Self::Output;
}

#[batch_impl(fn(u8) -> u16: [fn(u8) -> u16, fn(u16) -> u32])]
impl Invoke for fn(u8) -> u16 {
    type Input = u8;
    type Output = u16;
    fn invoke(&self, value: u8) -> u16 {
        self(value)
    }
}

#[batch_impl(fn(u8) -> u16 impl{fn(A) -> B} #call{self(x)})]
trait AttributeFn {
    fn call(&self, x: u8) -> u16;
}

trait FunctionLabels {}

#[batch_impl(fn(u8: u8): fn(Vec<u8>))]
impl FunctionLabels for fn(u8: u8) {}

trait NestedFunctionLabels {}

trait TraitFunctionLabels<F> {}

#[batch_impl(u8: Vec<u8>)]
impl TraitFunctionLabels<fn(u8: u8)> for u8 {}

#[batch_impl(
    fn(u8: fn(u8: u8)) -> fn(u8: u8): fn(fn(Vec<u8>)) -> fn(Vec<u8>)
)]
impl NestedFunctionLabels for fn(u8: fn(u8: u8)) -> fn(u8: u8) {}

#[test]
fn callable_prototypes_adapt_parameter_and_return_types() {
    let first: fn(u8) -> u16 = |x| u16::from(x) + 1;
    let second: fn(u16) -> u32 = |x| u32::from(x) + 2;
    assert_eq!(Invoke::invoke(&first, 4), 5);
    assert_eq!(Invoke::invoke(&second, 7), 9);
    assert_eq!(AttributeFn::call(&first, 10), 11);
    fn labeled<T: FunctionLabels>() {}
    fn nested<T: NestedFunctionLabels>() {}
    fn trait_argument<T: TraitFunctionLabels<fn(Vec<u8>)>>() {}
    labeled::<fn(Vec<u8>)>();
    nested::<fn(fn(Vec<u8>)) -> fn(Vec<u8>)>();
    trait_argument::<Vec<u8>>();
}
