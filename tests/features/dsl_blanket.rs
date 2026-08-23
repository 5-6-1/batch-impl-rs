//! dsl.rs blanket delegation core: `#blanket` `@0` position markers, deref
//! delegation, `:N` depth, lazy constants + generic traits, by-value
//! receivers, attr wrapper chains.
//! (split from the former single-file `tests/dsl.rs`)

use batch_impl::{batch_impl, batch_impl_only, batch_trait};
use std::rc::Rc;

// A blanket wrapper whose main part contains `@0` marks the target position:
// `@0` is replaced by the fresh target generic and the wrapper is emitted
// as-is (so `T` can sit anywhere, e.g. `(u32, @0, u8)`); without `@0` the
// wrapper is applied as `wrapper.T` (target appended last).
trait BlanketAt0 {
    fn tag(&self) -> u32;
}
#[batch_impl_only(u32 { fn tag(&self) -> u32 { 7 } })]
trait BlanketAt0 {}
#[batch_impl_only(#blanket(@all_methods){Box<@0>})]
trait BlanketAt0 {
    fn tag(&self) -> u32;
}

#[test]
fn blanket_at0_position() {
    let b = Box::new(5u32);
    assert_eq!(<Box<u32> as BlanketAt0>::tag(&b), 7);
}

// `@0` combines with user generics: a custom Deref type with a const
// parameter — `<const N: usize> #blanket(@all){MyPtrWithNum<@0, N>}` keeps
// the user's `N` and replaces `@0` with the fresh target generic; the
// delegation body derefs one layer (`**self`) to the `T` target.
struct MyPtrWithNum<T, const N: usize>(Box<T>, [u8; N]);
impl<T, const N: usize> std::ops::Deref for MyPtrWithNum<T, N> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}
trait CfgT {
    fn tag(&self) -> u32;
}
#[batch_impl_only(u32 { fn tag(&self) -> u32 { 7 } })]
trait CfgT {}
#[batch_impl_only(<const N: usize> #blanket(@all){MyPtrWithNum<@0, N>})]
trait CfgT {
    fn tag(&self) -> u32;
}

#[test]
fn blanket_at0_const_generic() {
    let p = MyPtrWithNum(Box::new(5u32), [0u8; 4]);
    assert_eq!(<MyPtrWithNum<u32, 4> as CfgT>::tag(&p), 7);
}

// ============================================================
// 36. #blanket overriding delegation (implement the inner type first, then wrap it)
// ============================================================
#[batch_impl(u32 { fn name(&self) -> String { self.to_string() } })]
#[batch_impl(#blanket(@all){&,Box,Rc})]
trait BlanketName {
    fn name(&self) -> String;
}

#[batch_impl(u16 { fn inc(&mut self) -> u16 { *self += 1; *self } })]
#[batch_impl(#blanket(inc){&mut})]
trait BlanketInc {
    fn inc(&mut self) -> u16;
}

// Nested wrapping and `:N` depth annotations: `Box.Rc:2` → `Box<Rc<T>>` (delegates `***self`),
// `Box.Box.Box:3` → `Box<Box<Box<T>>>` (delegates `****self`)
#[batch_impl(u32 { fn deep(&self) -> u32 { *self } })]
#[batch_impl(#blanket(deep){Box.Rc:2, Box.Box.Box:3})]
trait BlanketDeep {
    fn deep(&self) -> u32;
}

#[test]
fn blanket_delegate() {
    let v = 42u32;
    assert_eq!(v.name(), "42");
    assert_eq!(Box::new(7u32).name(), "7");
    assert_eq!(Rc::new(9u32).name(), "9");

    let mut b = Box::new(2u16);
    b.inc(); // Derefs to u16's own impl
    assert_eq!(*b, 3);

    // BlanketInc's blanket `&mut` delegation path (`impl<T: BlanketInc> BlanketInc for &mut T`;
    // `&mut u16` matches both u16's own impl and the blanket impl, requiring UFCS disambiguation)
    let mut x = 2u16;
    let mut xr: &mut u16 = &mut x;
    BlanketInc::inc(&mut xr); // delegates (**self).inc() → u16's own impl
    assert_eq!(x, 3);

    let br: Box<Rc<u32>> = Box::new(Rc::new(1u32));
    assert_eq!(br.deep(), 1);
    let bbb: Box<Box<Box<u32>>> = Box::new(Box::new(Box::new(2u32)));
    assert_eq!(bbb.deep(), 2);
}

// ============================================================
// 37. Lazy expansion (constant values with DSL ops / chained references) + blanket generic traits / assoc delegation
// ============================================================
trait LazyA {}
trait LazyB {}
batch_trait!(
    @lazy_nums=[u8, u16];
    @lazy_wrapped=[Box, Rc].@lazy_nums;
    @lazy_chain=@lazy_wrapped;
    LazyA: @lazy_chain;
    LazyB: @lazy_nums;
);

// blanket generic trait: params copied verbatim + where passed through + type/const projection delegation (@all)
#[batch_impl(Foo<u32> u32 {
    type Item = u8;
    const LIMIT: usize = 42;
    fn m(&self) -> u32 { *self }
})]
#[batch_impl(#blanket(@all){&})]
trait Foo<X: Clone>
where
    X: Send,
{
    type Item;
    const LIMIT: usize;
    fn m(&self) -> X;
}

#[test]
fn lazy_const_and_generic_blanket() {
    fn _a<T: LazyA>(_: &T) {}
    _a(&Box::new(0u8));
    _a(&Rc::new(0u16));
    fn _b<T: LazyB>(_: &T) {}
    _b(&0u8);
    _b(&0u16);

    assert_eq!(<u32 as Foo<u32>>::m(&5u32), 5);
    assert_eq!(<&u32 as Foo<u32>>::m(&&5u32), 5); // blanket delegation
    assert_eq!(<&u32 as Foo<u32>>::LIMIT, 42); // const projection
    let _: <&u32 as Foo<u32>>::Item = 8u8; // type projection
}

// #blanket with a by-value receiver: the generated impls carry a #[doc]
// note (warnings have no stable channel) — generation and type-checking are
// unchanged; Box's `**self` move-out type-checks here, `&` wrappers would not.
#[batch_impl(#blanket(@all_methods){Box})]
trait ConsumeAll {
    fn consume(self);
    fn len(&self) -> usize;
}

impl ConsumeAll for u8 {
    fn consume(self) {}
    fn len(&self) -> usize {
        1
    }
}

#[test]
fn blanket_by_value_receiver() {
    fn _c<T: ConsumeAll>(_: &T) {}
    _c(&Box::new(0u8));
    assert_eq!(Box::new(7u8).len(), 1);
    Box::new(9u8).consume(); // by-value forward: `(*self).consume()` moves out of the Box
}

// A method returning `Self::Assoc` (an associated-type projection) is
// blanket-able when the selection covers the associated item: the generated
// `type Output = <T as Trait>::Output;` makes the wrapper's `Self::Output`
// equal `T::Output`, so the forwarded call type-checks. Bare `Self` (the
// wrapper type itself) stays rejected (see `blanket_self_return`).
#[batch_impl(#blanket(@all){&, Box})]
trait AssocRet {
    type Output;
    fn get(&self) -> Self::Output;
}

impl AssocRet for u32 {
    type Output = u64;
    fn get(&self) -> u64 {
        *self as u64
    }
}

#[test]
fn blanket_self_assoc_return_covered() {
    assert_eq!(<&u32 as AssocRet>::get(&&5u32), 5);
    assert_eq!(<Box<u32> as AssocRet>::get(&Box::new(6u32)), 6);
    let _: <&u32 as AssocRet>::Output = 1u64;
}

// `Box@?` — the trailing `@?` suffix adds `where{T: ?Sized}` for that
// wrapper, so unsized targets work: `Box<dyn DynBox>` delegates through the
// trait object. Without `@?` the `T: Trait` bound would imply `Sized`.
#[batch_impl(#blanket(@all_methods){Box@?})]
trait DynBox {
    fn foo(&self) -> u32;
}

impl DynBox for u8 {
    fn foo(&self) -> u32 {
        *self as u32
    }
}

#[test]
fn blanket_unsized_wrapper() {
    let b: Box<dyn DynBox> = Box::new(7u8);
    assert_eq!(b.foo(), 7);
}

// `#[attr]` followed by an operator chain (not `.`-joined at the spec
// level): the attr's `apply` keeps an already-attached inner and applies
// the operator to it (`#[attr] Box.u8` = `#[attr] Box<u8>` — 0.7.2 fix:
// the inner was silently replaced).
#[batch_impl(#[allow(dead_code)] Box.u8)]
trait AttrChain {}

#[test]
fn attr_wrapper_chain() {
    fn _c<T: AttrChain>(_: &T) {}
    _c(&Box::new(0u8));
}
