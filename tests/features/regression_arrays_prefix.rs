//! regression.rs §19-22: array/slice builders (`TyPrimitiveArray`), list
//! distribution for attribute/prefix prefixes, `batch_trait!` `A<>`
//! passthrough, and `T^<A,B>` caret-after-angle-list.
//! (split from the former single-file `tests/regression.rs`)

use batch_impl::{batch_impl, batch_trait};
use std::collections::HashMap;

// ============================================================
// 19. Array/slice builder: `TyPrimitiveArray` merging TySlice + TyFixedArray
//     - `[]^T` => `[T]` (empty base wraps out a slice)
//     - `[T]^N` => `[T; N]` (numeric literal / const generic / range / list)
//     - `<const N> []-X-N` => `[X; N]`: the whole matrix wrapped into a const generic array
//     - `()^N` fresh generic tuples auto-extracted when used as generic args / array elements
// ============================================================
#[batch_impl([]^u8)]
trait ArrSlice {}

#[batch_impl([u8]^3)]
trait ArrLit {}

#[batch_impl(<const N: usize> [u8]^N)]
trait ArrConst {}

#[batch_impl([u8]^1..3)]
trait ArrRange {}

#[batch_impl([u8]^[1, 2, 4])]
trait ArrList {}

#[batch_impl(<const N: usize> []-[&, self, Box]^[u8, i8, ()^0..3]-N)]
trait ArrMatrix {}

#[batch_impl(Box^()^0..3)]
trait ArrTupleGeneric {}

#[test]
fn primitive_array_rules() {
    fn s<T: ArrSlice + ?Sized>(_: &T) {}
    fn l<T: ArrLit>(_: &T) {}
    fn c<T: ArrConst>(_: &T) {}
    fn r<T: ArrRange>(_: &T) {}
    fn ls<T: ArrList>(_: &T) {}
    fn m<T: ArrMatrix>(_: &T) {}
    fn tg<T: ArrTupleGeneric>(_: &T) {}

    s(&[1u8, 2][..]);
    l(&[0u8; 3]);
    c(&[0u8; 7]);
    r(&[0u8; 1]);
    r(&[0u8; 2]);
    ls(&[0u8; 1]);
    ls(&[0u8; 4]);
    m(&[&5u8; 2]);
    m(&[5i8; 2]);
    m(&[(); 2]);
    m(&[(1u8, 2i8); 2]);
    let bx: [Box<u8>; 2] = [Box::new(1), Box::new(2)];
    m(&bx);
    tg(&Box::new(()));
    tg(&Box::new((1u8,)));
    tg(&Box::new((1u8, 2u16)));
}

// ============================================================
// 20. List distribution for attribute/prefix prefixes: `#[attr] [A, B]` / `& [A, B]`
//     must be distributed via the top-level array (otherwise the list would be treated as a
//     whole type, producing an illegal `[A, B]` target)
// ============================================================
#[batch_impl(#[allow(dead_code)] [u8, u16])]
trait AttrDistribute {}

#[batch_impl(& [u8, u16])]
trait RefDistribute {}

#[batch_impl(#[allow(dead_code)] [u8, u16] { fn t(&self) -> &'static str { "x" } })]
trait AttrBodyDistribute {
    fn t(&self) -> &'static str;
}

#[batch_impl(& [u8, u16] { fn t(&self) -> &'static str { "y" } })]
trait RefBodyDistribute {
    fn t(&self) -> &'static str;
}

#[test]
fn prefix_attr_list_distribution() {
    fn a<T: AttrDistribute>(_: &T) {}
    fn r<T: RefDistribute>(_: &T) {}
    a(&0u8);
    a(&0u16);
    r(&(&0u8));
    r(&(&0u16));
    assert_eq!(AttrBodyDistribute::t(&0u8), "x");
    assert_eq!(AttrBodyDistribute::t(&0u16), "x");
    assert_eq!((&&0u8).t(), "y");
    assert_eq!((&&0u16).t(), "y");
}

// ============================================================
// 21. `batch_trait!` `A<>` passthrough (no trait definition, empty args passed through verbatim)
//     (`#[batch_impl]` copies the trait generics for `A<>`; `batch_trait!` has no definition
//     to copy, so `GA<>` keeps empty args and renders as `GA` — this case locks in the passthrough)
// ============================================================
trait PassGen {}

batch_trait!(PassGen: PassGen<> ());

#[test]
fn batch_trait_empty_angle_passthrough() {
    fn _check<T: PassGen>() {}
    _check::<()>();
}

// ============================================================
// 22. `T^<A,B>` caret followed by a generic argument list (legacy syntax case)
//     (parse_primary's `[Group] → parse_group` used to intercept a single angle-bracket
//     group first, swallowing the right operand and silently dropping `<u32, String>`,
//     outputting a bare `HashMap`; after the fix, the designed semantics apply:
//     `T^<A,B> => T<A,B>`)
// ============================================================
#[batch_impl(HashMap^<u32, String> { fn klen(&self) -> usize { self.len() } })]
trait CaretAngleList {
    fn klen(&self) -> usize;
}

#[test]
fn caret_angle_param_list() {
    let m: HashMap<u32, String> = HashMap::new();
    assert_eq!(m.klen(), 0);
    m.contains_key(&1u32); // ensure the impl lands on HashMap<u32, String> rather than a bare HashMap
}
