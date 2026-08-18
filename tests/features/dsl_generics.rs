//! dsl.rs generic-inheritance tests: automatic trait generic bound
//! inheritance (position + same name), `A<>` verbatim copying, and
//! trait-level where-clause inheritance (single-param merge / verbatim
//! passthrough / syn-AST reference collection).
//! (split from the former single-file `tests/dsl.rs`)

use batch_impl::batch_impl;
use std::collections::HashMap;

// ============================================================
// 32. Automatic trait generic bound inheritance: impl generic params without bounds inherit
//     by position + same name
//     (if written, the user is responsible; the macro does not interfere — sub-trait implication
//     (`trait B: A` makes `T: B` imply `T: A`) cannot be inferred by the macro and is left to
//     rustc; mismatched names error loudly, never silently)
// ============================================================
#[batch_impl(<T> Cloned<T> Vec<T> {
    fn get(&self) -> T {
        self[0].clone()
    }
})]
trait Cloned<T: Clone> {
    fn get(&self) -> T;
}

// User already wrote a bound (B: A implies T: A) → no intervention
trait SupA {}
trait SupB: SupA {}
struct SupS;
impl SupA for SupS {}
impl SupB for SupS {}
#[batch_impl(<T: SupB> Inherit<T> ())]
trait Inherit<T: SupA> {}

// Lifetime bound inheritance: `<'a, T>` → `impl<'a, T: 'a>`
#[batch_impl(<'a, T> Lifetime<'a, T> ())]
trait Lifetime<'a, T: 'a> {}

// Renaming scenario: lifetime renamed ('b vs 'a), trait name kept — the impl has no `'a`,
// so the lifetime bound is not inherited; the user writes `T: 'b` manually
#[batch_impl(<'b, T: 'b> LifetimeRenamed<'b, T> ())]
trait LifetimeRenamed<'a, T: 'a> {}

// `'static` is globally available: no declaration needed, inherited as usual
#[batch_impl(<T> StaticT<T> ())]
trait StaticT<T: 'static> {}

// Mixed bounds: Clone + 'a inherited together
#[batch_impl(<'a, T> Mix<'a, T> ())]
trait Mix<'a, T: Clone + 'a> {}

// Partial binding: T has a user-written bound (B implies A, verified by rustc), U has none
// (inherits A by name) — inheritance is decided per-parameter, so written/inherited mix naturally
#[batch_impl(<T: SupB, U> PartialBound<T, U> ())]
trait PartialBound<T: SupA, U: SupA> {}

#[batch_impl(<T, U: SupB> PartialBound2<T, U> ())]
trait PartialBound2<T: SupA, U: SupA> {}

impl SupA for i32 {}

#[test]
fn trait_bound_inherit() {
    let v: Vec<i32> = vec![42];
    assert_eq!(v.get(), 42);

    fn check<T: Inherit<SupS>>() {}
    check::<()>();

    fn check2<T: Lifetime<'static, ()>>() {}
    check2::<()>();

    fn check2r<T: LifetimeRenamed<'static, ()>>() {}
    check2r::<()>();

    fn check3<T: StaticT<()>>() {}
    check3::<()>();

    fn check4<T: Mix<'static, ()>>() {}
    check4::<()>();

    // Partial binding: impl<T: SupB, U: SupA> / impl<T: SupA, U: SupB>
    fn check_p<T: PartialBound<SupS, i32>>() {}
    check_p::<()>();
    fn check_p2<T: PartialBound2<i32, SupS>>() {}
    check_p2::<()>();
}

// ============================================================
// 33. `A<>`: trait generics copied verbatim — args and bounds all come from the trait
//     definition, expanding to `<'a, T: bounds, const N> A<'a, T, N>` (equivalent to writing it by hand)
// ============================================================
#[batch_impl(EmptyGenA<> ())]
trait EmptyGenA<T: Clone> {}

#[batch_impl(EmptyGenB<> ())]
trait EmptyGenB<'a, T: 'a> {}

#[batch_impl(EmptyGenC<> Vec<T>)]
trait EmptyGenC<T> {}

// `A<bounds>`: positional args copied verbatim + associated type bindings kept
// `AssocGen<Item=T>` → `<'T: Clone> AssocGen<T, Item = T>`
#[batch_impl(AssocGen<Item=T> ())]
trait AssocGen<T: Clone> {
    type Item;
}

#[batch_impl(AssocGen2<First=T, Second=U> ())]
trait AssocGen2<'a, T: Clone + 'a, U: Ord> {
    type First;
    type Second;
}

#[test]
fn empty_trait_generics() {
    fn check_a<T: EmptyGenA<i32>>() {}
    check_a::<()>();

    fn check_b<T: EmptyGenB<'static, ()>>() {}
    check_b::<()>();

    fn check_c<T: EmptyGenC<i32>>() {}
    check_c::<Vec<i32>>();

    fn check_d<T: AssocGen<i32, Item = i32>>() {}
    check_d::<()>();

    fn check_e<T: AssocGen2<'static, i32, u32, First = i32, Second = u32>>() {}
    check_e::<()>();
}

// ============================================================
// 34. Trait-level where clause inheritance: single-parameter predicates merge into bounds,
//     other predicates pass through verbatim
//     (`trait Foo<T> where T: Clone` → `impl<T: Clone>`;
//     composite predicates such as `T::Item: Clone` → the impl's where clause; `<T>` and `<>`
//     behave the same; reference collection happens on the syn AST: the B in `A::B` is an
//     associated type name, not misjudged as a parameter)
// ============================================================
#[batch_impl(<T> WhereCloned<T> Vec<T> {
    fn wget(&self) -> T {
        self[0].clone()
    }
})]
trait WhereCloned<T>
where
    T: Clone,
{
    fn wget(&self) -> T;
}

// where predicate + inline bound merging: T: Clone (inline) + T: Ord (where)
#[batch_impl(<T> WhereBoth<T> ())]
trait WhereBoth<T: Clone>
where
    T: Ord,
{
}

// Lifetime where predicate: `T: 'a`
#[batch_impl(<'a, T> WhereLifetime<'a, T> ())]
trait WhereLifetime<'a, T>
where
    T: 'a,
{
}

// Composite predicate `T::Item: Clone` passed through verbatim (`<T>` form)
#[batch_impl(<T> WhereGen<T> ())]
trait WhereGen<T: Clone>
where
    T: IntoIterator,
    T::Item: Clone,
{
}

// Same composite predicate (`A<>` verbatim form)
#[batch_impl(WhereGen2<> ())]
trait WhereGen2<T: Clone>
where
    T: IntoIterator,
    T::Item: Clone,
{
}

// Name collision: the B in `A::B` is an associated type name (not a parameter reference) —
// the impl declaring only A does not error
trait HasB {
    type B;
}
trait OtherTrait {}
struct S;
impl HasB for S {
    type B = u8;
}
impl OtherTrait for u8 {}

#[batch_impl(<A> ProjAssoc<A, u8> ())]
trait ProjAssoc<A, B>
where
    A: HasB,
    A::B: OtherTrait,
{
}

// const generic array predicate: the N in `[T; N]: Sized` is a const parameter reference
// (Expr position), and `A<>` verbatim automatically declares N
#[batch_impl(WhereArr<> ())]
trait WhereArr<T, const N: usize>
where
    [T; N]: Sized,
{
}

// Deep-recursion left side: tuple + generic args + qualified projection (the U in `<U as HasB2>::B`)
trait HasB2 {
    type B;
}
struct S2;
impl HasB2 for S2 {
    type B = u8;
}

#[batch_impl(Deep<> ())]
trait Deep<T, U>
where
    U: HasB2,
    Vec<(T, <U as HasB2>::B)>: Sized,
{
}

// Tuple predicate: A and B in `(A, B)` are both parameter references (multi-type dependency)
trait TupleT {
    type Assoc;
}
trait TupleT2 {}
impl TupleT for u8 {
    type Assoc = u8;
}
impl TupleT2 for (u8, u8) {}

#[batch_impl(TuplePred<> ())]
trait TuplePred<A, B>
where
    A: TupleT,
    (A, B): TupleT2,
{
}

// fn type predicate: the parameter/return types of `fn(A) -> B` are both reference positions
#[batch_impl(FnType<> ())]
trait FnType<A, B>
where
    fn(A) -> B: Sized,
{
}

// Reference predicate: both the lifetime and the type of `&'a T` are collected
#[batch_impl(RefPred<> ())]
trait RefPred<'a, T>
where
    T: 'a,
    &'a T: Sized,
{
}

// List distribution + composite predicates: each leaf does its own reference check
#[batch_impl(<T> ListPred2<T> [Vec<T>, <U> HashMap<T, U>])]
trait ListPred2<T>
where
    T: IntoIterator,
    T::Item: Clone,
{
}

#[test]
fn trait_where_clause_inherit() {
    let v: Vec<i32> = vec![42];
    assert_eq!(v.wget(), 42);

    fn check_b<T: WhereBoth<i32>>() {}
    check_b::<()>();

    fn check_l<T: WhereLifetime<'static, ()>>() {}
    check_l::<()>();

    fn check_g<T: WhereGen<Vec<i32>>>() {}
    check_g::<()>();
    fn check_g2<T: WhereGen2<Vec<i32>>>() {}
    check_g2::<()>();

    fn check_p<T: ProjAssoc<S, u8>>() {}
    check_p::<()>();

    fn check_a<T: WhereArr<u8, 4>>() {}
    check_a::<()>();

    fn check_d<T: Deep<S2, S2>>() {}
    check_d::<()>();

    fn check_t<T: TuplePred<u8, u8>>() {}
    check_t::<()>();

    fn check_f<T: FnType<u8, u8>>() {}
    check_f::<()>();

    fn check_r<T: RefPred<'static, u8>>() {}
    check_r::<()>();

    fn check_lp<T: ListPred2<Vec<i32>>>() {}
    check_lp::<Vec<Vec<i32>>>();
    check_lp::<HashMap<Vec<i32>, i32>>();
}
