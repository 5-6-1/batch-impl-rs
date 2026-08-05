// High-value regression tests for batch-impl.
//
// This collects the key corner cases and consistency checks pulled from the original
// `examples/tests.rs` (parts not covered by `tests/dsl.rs`):
// - nested angle brackets (`>>`)
// - path types (`std::collections::HashMap<K, V>`)
// - const generics mixed with type annotations
// - lifetime generics
// - dyn traits + multiple bounds
// - `batch_impl` vs `batch_trait!` consistency across 10 specs
// - passthrough of macro invocations `m![]`, and the boundary between macro bodies and directives / bare where

use batch_impl::{batch_impl, batch_impl_only, batch_trait};

// ============================================================
// 1. Nested angle brackets Vec<Vec<T>> — verify `>>` does not break depth tracking
// ============================================================
#[batch_impl(<T> Vec<Vec<T>>)]
trait NestedGeneric {}

#[test]
fn nested_angle_brackets() {
    fn _check<T: NestedGeneric>() {}
    _check::<Vec<Vec<i32>>>();
    _check::<Vec<Vec<String>>>();
}

// ============================================================
// 2. Path type std::collections::HashMap<K, V>
// ============================================================
#[batch_impl(<K, V> std::collections::HashMap<K, V>)]
trait PathType {}

#[test]
fn path_type_with_generics() {
    fn _check<T: PathType>() {}
    _check::<std::collections::HashMap<i32, String>>();
}

// ============================================================
// 3. const generic <const N: usize> [i32; N]
// ============================================================
#[batch_impl(<const N: usize> ConstGeneric<N> [i32; N] {
    fn len_const(&self) -> usize { N }
    fn first(&self) -> i32 { self[0] }
})]
trait ConstGeneric<const N: usize> {
    fn len_const(&self) -> usize;
    fn first(&self) -> i32;
}

#[test]
fn const_generic_array() {
    let arr: [i32; 5] = [10, 20, 30, 40, 50];
    assert_eq!(arr.len_const(), 5);
    assert_eq!(arr.first(), 10);
}

// ============================================================
// 4. Type annotations mixed with const generics <T: Clone, const N: usize>
//    Verify the DSL is not confused by spaces / commas in `<T : Clone , const N : usize>`
#[batch_impl(<T: Clone, const N: usize> MixedGeneric<T, N> [T; N] {
    fn repeat_inner(&self) -> Vec<T> {
        std::iter::repeat_n(self[0].clone(), N).collect()
    }
})]
trait MixedGeneric<T, const N: usize> {
    fn repeat_inner(&self) -> Vec<T>;
}

#[test]
fn mixed_type_bound_and_const_generic() {
    let arr: [String; 3] =
        [String::from("hi"), String::from("hi"), String::from("hi")];
    assert_eq!(arr.repeat_inner().len(), 3);
}

// ============================================================
// 5. Lifetime generics <'a, T: 'a> &'a T
// ============================================================
#[allow(dead_code)]
#[batch_impl(<'a, T: 'a> LifetimeTrait<'a, T> &'a T)]
trait LifetimeTrait<'a, T> {}

#[test]
fn lifetime_generic() {
    fn _check<'a, T: 'a>()
    where
        &'a T: LifetimeTrait<'a, T>,
    {
    }
    _check::<'static, i32>();
}

// ============================================================
// 6. dyn trait + multiple bounds (`+ Send + Sync`)
// ============================================================
#[batch_impl(dyn std::fmt::Display + Send + Sync)]
trait DynMarkerMultiBound {}

#[test]
fn dyn_trait_with_multi_bounds() {
    fn _check<T: DynMarkerMultiBound + ?Sized>() {}
    _check::<dyn std::fmt::Display + Send + Sync>();
}

// ============================================================
// 7. `batch_impl` vs `batch_trait!` consistency
//
// 10 parallel specs: the same DSL should generate equivalent impls under both macros.
// Compile-time checks + a few runtime assert_eqs.
// ============================================================

// --- basic types ---
trait CmpBase {}
#[batch_impl(usize)]
trait CmpAttrBase {}
batch_trait!(CmpBase: usize);

#[test]
fn cmp_basic() {
    fn _a<T: CmpAttrBase>() {}
    fn _b<T: CmpBase>() {}
    _a::<usize>();
    _b::<usize>();
}

// --- generics ---
trait CmpGeneric {}
#[batch_impl(<T> Vec<T>)]
trait CmpAttrGeneric {}
batch_trait!(CmpGeneric: <T> Vec<T>);

#[test]
fn cmp_generic() {
    fn _a<T: CmpAttrGeneric>() {}
    fn _b<T: CmpGeneric>() {}
    _a::<Vec<i32>>();
    _b::<Vec<i32>>();
}

// --- trait generics + custom body ---
trait CmpTraitGen<T> {
    fn wrap(val: T) -> Self;
}
#[batch_impl(<T> CmpAttrTraitGen<T> i32 {
    fn wrap(_val: T) -> Self { 0 }
})]
trait CmpAttrTraitGen<T> {
    fn wrap(val: T) -> Self;
}
batch_trait!(
    CmpTraitGen: <T> CmpTraitGen<T> i32 {
        fn wrap(_val: T) -> Self { 0 }
    }
);

#[test]
fn cmp_trait_generic_with_body() {
    let a: i32 = CmpAttrTraitGen::<String>::wrap(String::new());
    let b: i32 = CmpTraitGen::<String>::wrap(String::new());
    assert_eq!(a, 0);
    assert_eq!(b, 0);
}

// --- parallel lists ---
trait CmpList {
    fn tag(&self) -> &'static str;
}
#[batch_impl([u8, u16] { fn tag(&self) -> &'static str { "cmp" } })]
trait CmpAttrList {
    fn tag(&self) -> &'static str;
}
batch_trait!(
    CmpList: [u8, u16] { fn tag(&self) -> &'static str { "cmp" } }
);

#[test]
fn cmp_parallel_list() {
    assert_eq!(CmpAttrList::tag(&0u8), "cmp");
    assert_eq!(CmpList::tag(&0u16), "cmp");
}

// --- ^ operator (reference prefix) ---
trait CmpCaret {}
#[batch_impl(&^u32)]
trait CmpAttrCaret {}
batch_trait!(CmpCaret: &^u32);

#[test]
fn cmp_caret_prefix() {
    fn _a<T: CmpAttrCaret>() {}
    fn _b<T: CmpCaret>() {}
    _a::<&u32>();
    _b::<&u32>();
}

// --- nested ^ ---
trait CmpNestedCaret {}
#[batch_impl(Box^Box^isize)]
trait CmpAttrNestedCaret {}
batch_trait!(CmpNestedCaret: Box^Box^isize);

#[test]
fn cmp_nested_caret() {
    fn _a<T: CmpAttrNestedCaret>() {}
    fn _b<T: CmpNestedCaret>() {}
    _a::<Box<Box<isize>>>();
    _b::<Box<Box<isize>>>();
}

// --- ^ through [] ---
trait CmpCaretBracket {}
#[batch_impl(Box^[Box^isize])]
trait CmpAttrCaretBracket {}
batch_trait!(CmpCaretBracket: Box^[Box^isize]);

#[test]
fn cmp_caret_through_bracket() {
    fn _a<T: CmpAttrCaretBracket>() {}
    fn _b<T: CmpCaretBracket>() {}
    _a::<Box<[Box<isize>]>>();
    _b::<Box<[Box<isize>]>>();
}

// --- const generics ---
trait CmpConst<const N: usize> {
    fn val() -> usize {
        N
    }
}
#[batch_impl(<const N: usize> CmpAttrConst<N> [i32; N])]
trait CmpAttrConst<const N: usize> {
    fn val() -> usize {
        N
    }
}
batch_trait!(CmpConst: <const N: usize> CmpConst<N> [i32; N]);

#[test]
fn cmp_const_generic() {
    let a = <[i32; 5] as CmpAttrConst<5>>::val();
    let b = <[i32; 5] as CmpConst<5>>::val();
    assert_eq!(a, 5);
    assert_eq!(b, 5);
}

// --- lifetimes ---
#[allow(dead_code)]
trait CmpLifetime<'a, T> {}
#[allow(dead_code)]
#[batch_impl(<'a, T: 'a> CmpAttrLifetime<'a, T> &'a T)]
trait CmpAttrLifetime<'a, T> {}
batch_trait!(CmpLifetime: <'a, T: 'a> CmpLifetime<'a, T> &'a T);

#[test]
fn cmp_lifetime() {
    fn _a<'a, T: 'a>()
    where
        &'a T: CmpAttrLifetime<'a, T>,
    {
    }
    fn _b<'a, T: 'a>()
    where
        &'a T: CmpLifetime<'a, T>,
    {
    }
    // compiling is enough
    let _ = _a::<'static, i32>;
    let _ = _b::<'static, i32>;
}

// --- path traits ---
mod cmp_mod {
    pub trait PathTrait {}
}
trait CmpPath {}
#[batch_impl(u32)]
trait CmpAttrPath {}
batch_trait!(CmpPath: u32; cmp_mod::PathTrait: u32);

#[test]
fn cmp_path_trait() {
    fn _a<T: CmpAttrPath>() {}
    fn _b<T: CmpPath>() {}
    fn _c<T: cmp_mod::PathTrait>() {}
    _a::<u32>();
    _b::<u32>();
    _c::<u32>();
}

// ============================================================
// 17. Macro invocations m![] interacting with the DSL
//     - m![] as a target type / generic argument / where predicate passthrough
//     - the m![] macro body is a passthrough macro argument: directives (#name) and bare
//       where must not enter the macro body
// ============================================================
macro_rules! ty {
    () => { Vec<u8> };
}
macro_rules! passthrough {
    ($($t:tt)*) => { $($t)* };
}

#[batch_impl(ty![])]
trait MacroBracketA {}
#[batch_impl(passthrough![Vec<u8>])]
trait MacroBracketB {}
#[batch_impl(Box<ty![]>)]
trait MacroBracketC {}

#[batch_impl(
    <T> MacroBracketFnRet<T> Vec<T> where T: Fn() -> ty![]
    { fn ok(&self) -> bool { true } }
)]
trait MacroBracketFnRet<T> {
    fn ok(&self) -> bool;
}

#[test]
fn macro_bracket_passthrough() {
    fn a<T: MacroBracketA>(_: &T) {}
    fn b<T: MacroBracketB>(_: &T) {}
    fn c<T: MacroBracketC>(_: &T) {}
    a(&vec![1u8]);
    b(&vec![1u8]);
    c(&Box::new(vec![1u8]));
    let v: Vec<fn() -> Vec<u8>> = vec![|| vec![1u8]];
    assert!(v.ok());
}

// --- m![] macro bodies do not expand directives and do not process bare where ---
trait MacroBracketDirective {
    fn len(&self) -> usize;
}

macro_rules! len_ty {
    (#len{ $n:expr }) => {
        u8
    };
}

#[batch_impl_only(
    usize #len{5},
    len_ty![#len{5}] #len{6}
)]
trait MacroBracketDirective {
    fn len(&self) -> usize;
}

#[test]
fn macro_bracket_directive_not_expanded() {
    assert_eq!(0usize.len(), 5);
    assert_eq!(0u8.len(), 6);
}

trait MacroBracketWhere<T> {
    fn ok2(&self) -> bool;
}

macro_rules! m2 {
    (where) => { Vec<u8> };
}

#[batch_impl_only(
    <T> MacroBracketWhere<T> Vec<T> where T: Fn() -> m2![where]
    { fn ok2(&self) -> bool { true } }
)]
trait MacroBracketWhere<T> {
    fn ok2(&self) -> bool;
}

#[test]
fn macro_bracket_where_not_processed() {
    let v: Vec<fn() -> Vec<u8>> = vec![|| vec![1u8]];
    assert!(v.ok2());
}

// ============================================================
// 18. Path prefix `#path::to::Trait:` (batch_impl_only)
//     - the generated impl references a real trait in an external module
//     - the dummy trait is still used to read directive signatures
//     - `Trait<T>` in the DSL is recognized as a trait generic application by the trailing
//       ident of the path
// ============================================================
mod ext {
    pub mod traits {
        pub trait PathPrefixTrait {
            fn tag(&self) -> &'static str;
        }

        pub trait PathPrefixGen<T> {
            fn head(&self) -> T;
        }
    }
}

// the dummy trait is discarded by batch_impl_only; import the real trait here so methods can be called
use ext::traits::{PathPrefixGen, PathPrefixTrait};

#[batch_impl_only(
    #ext::traits::PathPrefixTrait: usize #tag{"usize"}, isize #tag{"isize"}
)]
trait PathPrefixTrait {
    fn tag(&self) -> &'static str;
}

#[test]
fn cmp_path_prefix_directive() {
    assert_eq!(0usize.tag(), "usize");
    assert_eq!(0isize.tag(), "isize");
}

#[batch_impl_only(
    #ext::traits::PathPrefixGen: <T: Clone> PathPrefixGen<T> Vec<T>
    { fn head(&self) -> T { self[0].clone() } }
)]
trait PathPrefixGen<T> {
    fn head(&self) -> T;
}

#[test]
fn cmp_path_prefix_trait_generic() {
    assert_eq!(vec![1i32].head(), 1);
    assert_eq!(vec![String::from("x")].head(), "x");
}

// path prefix + #blanket: the delegated bound must use the full external path
// (a bare dummy name cannot be resolved)
#[batch_impl_only(
    #ext::traits::PathPrefixTrait: u8 #tag{"u8"},
    #blanket(@all){&, Box}
)]
trait PathPrefixTrait {
    fn tag(&self) -> &'static str;
}

#[test]
fn cmp_path_prefix_blanket() {
    // `&u8` matches both u8's own impl and the blanket `&T` impl, requiring UFCS disambiguation
    assert_eq!(<&u8 as PathPrefixTrait>::tag(&&0u8), "u8");
    assert_eq!(Box::new(1u8).tag(), "u8");
}

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
use std::collections::HashMap;

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
