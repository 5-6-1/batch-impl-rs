//! Composite variadic rows keep their captures paired and scoped to a segment.

use batch_impl::batch_impl;
use core::any::type_name;
use std::rc::Rc;

#[batch_impl(
    (*[Vec, Rc, Box] *@u*,) impl{(A<B>@..,)}
    #pairs { vec![@((type_name::<@A<@B>>(), type_name::<@B>())),..] }
)]
trait ConstructorRows {
    fn pairs() -> Vec<(&'static str, &'static str)>;
}

#[test]
fn all_constructor_rows_keep_their_element_types_and_order() {
    type All = (
        Vec<u8>,
        Rc<u8>,
        Box<u8>,
        Vec<u16>,
        Rc<u16>,
        Box<u16>,
        Vec<u32>,
        Rc<u32>,
        Box<u32>,
        Vec<u64>,
        Rc<u64>,
        Box<u64>,
        Vec<u128>,
        Rc<u128>,
        Box<u128>,
        Vec<usize>,
        Rc<usize>,
        Box<usize>,
    );
    let mut expected = vec![];
    macro_rules! family {
        ($ty:ty) => {
            expected.extend([
                (type_name::<Vec<$ty>>(), type_name::<$ty>()),
                (type_name::<Rc<$ty>>(), type_name::<$ty>()),
                (type_name::<Box<$ty>>(), type_name::<$ty>()),
            ]);
        };
    }
    family!(u8);
    family!(u16);
    family!(u32);
    family!(u64);
    family!(u128);
    family!(usize);
    assert_eq!(All::pairs(), expected);
}

#[batch_impl(
    [(Vec<u8>, Rc<u16>), ()] impl{(A<B>@..,)}
    #pairs { vec![@A((type_name::<@A<@B>>(), type_name::<@B>())),..] }
)]
trait SparseRows {
    fn pairs() -> Vec<(&'static str, &'static str)>;
}

#[test]
fn a_sparse_target_visits_only_its_two_existing_rows() {
    assert_eq!(
        <(Vec<u8>, Rc<u16>) as SparseRows>::pairs(),
        [(type_name::<Vec<u8>>(), "u8"), (type_name::<Rc<u16>>(), "u16")]
    );
}

#[test]
fn empty_segments_still_declare_their_capture_names() {
    assert!(<() as SparseRows>::pairs().is_empty());
}

struct Pair<A, B>(A, B);

#[batch_impl(
    (Pair<String, Vec<String>>, Pair<Vec<u8>, Vec<Vec<u8>>>)
    impl{(Pair<T, Vec<T>>@..,)}
    #seed { @T(self.@0.1.push(self.@0.0.clone());).. }
    #types { vec![@T(type_name::<@Pair<@T, @Vec<@T>>>()),..] }
)]
trait PairedRows {
    fn seed(&mut self);
    fn types() -> Vec<&'static str>;
}

#[test]
fn repeated_names_are_equal_within_a_row_and_independent_between_rows() {
    type Columns = (Pair<String, Vec<String>>, Pair<Vec<u8>, Vec<Vec<u8>>>);
    let mut columns: Columns = (Pair(String::from("row"), vec![]), Pair(vec![7], vec![]));
    columns.seed();
    assert_eq!(columns.0.1, ["row"]);
    assert_eq!(columns.1.1, [vec![7]]);
    assert_eq!(
        Columns::types(),
        [type_name::<Pair<String, Vec<String>>>(), type_name::<Pair<Vec<u8>, Vec<Vec<u8>>>>()]
    );
}

#[batch_impl(
    (Vec<u8>, Rc<u16>, Box<u32>, Vec<u64>) impl{(Vec<T>@.., Vec<U>@..,)}
    #elements { vec![@((type_name::<@T>(), type_name::<@U>())),..] }
    #first { vec![@T(type_name::<@Vec<@T>>()),..] }
    #second { vec![@U(type_name::<@Vec<@U>>()),..] }
)]
trait SharedHeads {
    fn elements() -> Vec<(&'static str, &'static str)>;
    fn first() -> Vec<&'static str>;
    fn second() -> Vec<&'static str>;
}

#[test]
fn shared_names_are_local_to_the_selected_segment() {
    type Columns = (Vec<u8>, Rc<u16>, Box<u32>, Vec<u64>);
    assert_eq!(Columns::elements(), [("u8", "u32"), ("u16", "u64")]);
    assert_eq!(Columns::first(), [type_name::<Vec<u8>>(), type_name::<Rc<u16>>()]);
    assert_eq!(Columns::second(), [type_name::<Box<u32>>(), type_name::<Vec<u64>>()]);
}

struct T;

#[batch_impl(
    (u8, Vec<T>) impl{(T, Vec<T>@..,)}
    #types { (type_name::<T>(), vec![@(type_name::<@T>()),..]) }
)]
trait FixedAndRow {
    fn types() -> (&'static str, Vec<&'static str>);
}

#[test]
fn a_fixed_slot_does_not_rewrite_an_inserted_row_capture() {
    assert_eq!(<(u8, Vec<T>) as FixedAndRow>::types(), ("u8", vec![type_name::<T>()]));
}

trait ImplRows {
    fn pairs() -> Vec<(&'static str, &'static str)>;
}

#[batch_impl(X: (Vec<u8>, Rc<u16>) impl{(A<B>@..,)})]
impl ImplRows for X {
    fn pairs() -> Vec<(&'static str, &'static str)> {
        fresh!(vec![@A((type_name::<@A<@B>>(), type_name::<@B>())),..])
    }
}

#[test]
fn impl_entry_fresh_uses_the_same_composite_rows() {
    assert_eq!(
        <(Vec<u8>, Rc<u16>) as ImplRows>::pairs(),
        [(type_name::<Vec<u8>>(), "u8"), (type_name::<Rc<u16>>(), "u16")]
    );
}

#[batch_impl(
    (&'static u8, &'static u16) impl{(&'static Element@..,)}
    #sizes { vec![@(core::mem::size_of::<@Element>()),..] }
)]
trait ReferentRows {
    fn sizes() -> Vec<usize>;
}

#[batch_impl(
    ([u8; 3], [u16; 3]) impl{([Element; 3]@..,)}
    #sizes { vec![@(core::mem::size_of::<@Element>()),..] }
)]
trait ArrayRows {
    fn sizes() -> Vec<usize>;
}

#[test]
fn reference_and_array_patterns_capture_their_element_types() {
    assert_eq!(<(&'static u8, &'static u16) as ReferentRows>::sizes(), [1, 2]);
    assert_eq!(<([u8; 3], [u16; 3]) as ArrayRows>::sizes(), [1, 2]);
}
