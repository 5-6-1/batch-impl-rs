// Functional regression tests for the batch-impl DSL.
//
// These cases cover the core features: basic batch generation, generics, parallel
// lists, ^/- operators, tuple generation, unsafe impls, associated type bindings,
// fn types, attribute support, and the # directive system.
// examples/ keeps complete per-item println-style cases; this file provides
// `#[test]`-based core coverage.

use batch_impl::{batch_impl, batch_impl_only, batch_preprocess_test, batch_trait};
use std::collections::HashMap;
use std::rc::Rc;

// ============================================================
// 1. Basic: implement directly for concrete types
// ============================================================
#[batch_impl(usize, isize)]
trait Numeric {}

#[test]
fn basic_numeric() {
    fn check<T: Numeric>(_: &T) {}
    check(&0usize);
    check(&0isize);
}

// ============================================================
// 2. Generics: <T> Vec<T>
// ============================================================
#[batch_impl(<T> Vec<T>)]
trait Collection {}

#[test]
fn generic_vec() {
    fn check<T: Collection>(_: &T) {}
    check(&vec![1, 2, 3]);
    check(&vec!["a", "b"]);
}

// ============================================================
// 3. Shared + independent body merging
// ============================================================
#[batch_impl(
    [usize { fn name() -> &'static str { "usize" } },
     isize { fn name() -> &'static str { "isize" } }]
    { fn zero() -> Self { 0 } }
)]
trait Zero {
    fn zero() -> Self;
    fn name() -> &'static str;
}

#[test]
fn shared_independent_body() {
    assert_eq!(usize::zero(), 0);
    assert_eq!(isize::zero(), 0);
    assert_eq!(<usize as Zero>::name(), "usize");
    assert_eq!(<isize as Zero>::name(), "isize");
}

// ============================================================
// 4. ^ operator: [&, Box, Rc]^u32 cartesian product
// ============================================================
#[batch_impl([&, Box, Rc]^u32)]
trait RefOrOwnedEmpty {}

#[test]
fn caret_prefix_list() {
    fn check<T: RefOrOwnedEmpty>(_: &T) {}
    let v: u32 = 5;
    check(&(&v));
    check(&Box::new(v));
    check(&Rc::new(v));
}

// ============================================================
// 5. Tuple generation: ()^3
// ============================================================
#[batch_impl(()^3)]
trait Tuple3 {}

#[test]
fn tuple_pow_basic() {
    fn check<T: Tuple3>(_: &T) {}
    check(&(1u8, 2u16, 3u32));
}

// ============================================================
// 6. Range tuples: ()^1..=3
// ============================================================
#[batch_impl(()^1)]
trait Tuple1 {}
#[batch_impl(()^2)]
trait Tuple2 {}
#[batch_impl(()^3)]
trait Tuple3R {}

#[test]
fn tuple_range_pow() {
    fn t1<T: Tuple1>(_: &T) {}
    fn t2<T: Tuple2>(_: &T) {}
    fn t3<T: Tuple3R>(_: &T) {}
    t1(&(1u8,));
    t2(&(1u8, 2u16));
    t3(&(1u8, 2u16, 3u32));
}

// ============================================================
// 7. Associated type bindings: <T> Iter<Item=T> Vec<T> {...}
// ============================================================
#[batch_impl(<T> Iter<Item=T> Vec<T> {
    fn count(&self) -> usize { self.len() }
})]
trait Iter {
    type Item;
    fn count(&self) -> usize;
}

#[test]
fn assoc_type_binding() {
    assert_eq!(vec![1, 2, 3].count(), 3);
}

// ============================================================
// 8. unsafe impl: `unsafe` before TRAIT makes all impls unsafe
// ============================================================
/// # Safety
///
/// Marker trait for testing; no actual unsafe semantics.
#[batch_impl(usize, Box<u32>)]
unsafe trait UnsafeAll {}

#[test]
fn unsafe_trait_impls() {
    fn check<T: UnsafeAll>(_: &T) {}
    check(&0usize);
    check(&Box::new(0u32));
}

// ============================================================
// 9. Partial unsafe
// ============================================================
/// # Safety
///
/// Marker trait for testing; no actual unsafe semantics.
#[batch_impl(unsafe^usize, isize)]
unsafe trait PartialUnsafe {}

#[test]
fn partial_unsafe() {
    fn check<T: PartialUnsafe>(_: &T) {}
    check(&0usize);
    check(&0isize);
}

// ============================================================
// 10. fn types
// ============================================================
#[batch_impl(fn^(i32, u32))]
trait FnSimple {}

#[batch_impl(fn(i32, u32)-String)]
trait FnWithReturn {}

#[test]
fn fn_types() {
    fn check_simple<T: FnSimple>(_: &T) {}
    fn check_ret<T: FnWithReturn>(_: &T) {}
    let f: fn(i32, u32) = |_, _| {};
    check_simple(&f);
    let fr: fn(i32, u32) -> String = |_, _| String::new();
    check_ret(&fr);
}

// ============================================================
// 11. Attribute support: #[allow(dead_code)]^usize
// ============================================================
#[batch_impl(#[allow(dead_code)]^usize, isize)]
trait AttrSimple {}

#[test]
fn attr_support() {
    fn check<T: AttrSimple>(_: &T) {}
    check(&0usize);
    check(&0isize);
}

// ============================================================
// 12. Complex type passthrough
// ============================================================
#[batch_impl(
    (i32, String),
    &str,
    Box<dyn std::fmt::Display>,
    fn(i32) -> bool,
    dyn Fn() + Send + Sync
)]
trait ComplexMarker {}

#[test]
fn complex_passthrough() {
    fn check<T: ComplexMarker + ?Sized>(_: &T) {}
    check(&(1i32, String::from("x")));
    check(&"hi");
    let bd: Box<dyn std::fmt::Display> = Box::new(1i32);
    check(&bd);
    let ft: fn(i32) -> bool = |_| true;
    check(&ft);
    fn _dyn_check<T: ComplexMarker + ?Sized>() {}
    _dyn_check::<dyn Fn() + Send + Sync>();
}

// ============================================================
// 13. #name{body} single-item assignment
// ============================================================
#[batch_impl(
    usize #to_str{"usize"},
    isize #to_str{"isize"}
)]
trait IdentToString {
    fn to_str(&self) -> &'static str;
}

#[test]
fn directive_single_name() {
    assert_eq!(0usize.to_str(), "usize");
    assert_eq!(0isize.to_str(), "isize");
}

// ============================================================
// 14. #fill(args){body} multiple methods sharing one body
// ============================================================
#[batch_impl(usize #fill(name, kind){"u"})]
trait Describable {
    fn name(&self) -> &'static str;
    fn kind(&self) -> &'static str;
}

#[test]
fn directive_fill() {
    assert_eq!(0usize.name(), "u");
    assert_eq!(0usize.kind(), "u");
}

// ============================================================
// 15. #delegate delegation
// ============================================================
#[batch_impl(
    Vec<u32> #d_len{self.len()},
    Box^Vec^u32 #delegate(d_len){**self}
)]
trait MyLen {
    fn d_len(&self) -> usize;
}

#[test]
fn directive_delegate() {
    let v: Vec<u32> = vec![1, 2, 3];
    assert_eq!(v.d_len(), 3);
    let b: Box<Vec<u32>> = Box::new(vec![1, 2, 3, 4]);
    assert_eq!(b.d_len(), 4);
}

// `#delegate` auto-names `_` wildcard params (`arg0`, ...) so they can be
// forwarded — trait declarations may use `_` (a pattern parameter would be
// E0642), and a delegation call cannot forward an unnamed param.
trait WildcardInner {
    fn m(&self, ab: (u32, u32)) -> u32;
}
impl WildcardInner for Vec<u32> {
    fn m(&self, ab: (u32, u32)) -> u32 {
        ab.0 + ab.1
    }
}
#[batch_impl(Box<Vec<u32>> #delegate(@all_methods){**self})]
trait WildcardOuter {
    fn m(&self, _: (u32, u32)) -> u32;
}

#[test]
fn delegate_wildcard_param() {
    let b = Box::new(vec![1u32, 2]);
    assert_eq!(<Box<Vec<u32>> as WildcardOuter>::m(&b, (3, 4)), 7);
}

// A trait method with a default body may use a tuple pattern parameter
// (pattern params are only illegal in bodyless declarations, E0642);
// `#delegate` renames `(a, b)` to `arg0` and forwards it.
#[allow(dead_code)]
struct DestrBox(usize);
#[allow(dead_code)]
trait DestructureNum {
    fn dm(&self, ab: (u32, u32)) -> u32;
}
impl DestructureNum for DestrBox {
    fn dm(&self, ab: (u32, u32)) -> u32 {
        ab.0 * ab.1
    }
}
#[batch_impl(Box<DestrBox> #delegate(dm){**self})]
trait DestructureOuter {
    fn dm(&self, (a, b): (u32, u32)) -> u32 {
        a + b
    }
}

#[test]
fn delegate_tuple_pattern() {
    let b = Box::new(DestrBox(5));
    assert_eq!(<Box<DestrBox> as DestructureOuter>::dm(&b, (3, 4)), 12);
}

// A nested non-forwardable pattern (`(ref a, ref b)` — `ref` tokens cannot
// appear in an expression position) falls back to `arg{i}` renaming; a plain
// `(a, b)` keeps its pattern.
struct RefBox;
trait RefNum {
    fn rm(&self, ab: (u32, u32), extra: &u32) -> u32;
}
impl RefNum for RefBox {
    fn rm(&self, ab: (u32, u32), extra: &u32) -> u32 {
        ab.0 + ab.1 + *extra
    }
}
#[batch_impl(Box<RefBox> #delegate(rm){**self})]
trait RefOuter {
    fn rm(&self, (ref _a, ref _b): (u32, u32), _extra: &u32) -> u32 {
        0
    }
}

#[test]
fn delegate_ref_nested_pattern() {
    let b = Box::new(RefBox);
    assert_eq!(<Box<RefBox> as RefOuter>::rm(&b, (3, 4), &5), 12);
}

// A blanket wrapper whose main part contains `@0` marks the target position:
// `@0` is replaced by the fresh target generic and the wrapper is emitted
// as-is (so `T` can sit anywhere, e.g. `(u32, @0, u8)`); without `@0` the
// wrapper is applied as `wrapper^T` (target appended last).
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
// 16. batch_trait! function-like macro
// ============================================================
trait BTNumeric {}
trait BTMap {}

batch_trait!(
    BTNumeric: u8, u16, u32, u64;
    BTMap: HashMap<i32, i32>
);

#[test]
fn batch_trait_macro_basic() {
    fn check_num<T: BTNumeric>(_: &T) {}
    fn check_map<T: BTMap>(_: &T) {}
    check_num(&0u8);
    check_num(&0u16);
    check_num(&0u32);
    check_num(&0u64);
    check_map(&HashMap::<i32, i32>::new());
}

// ============================================================
// 17. batch_trait! multi-segment + unsafe segment
// ============================================================
trait PairSegment {}

batch_trait!(
    PairSegment: usize, isize;
    unsafe YieldUnsafe: u32
);

/// # Safety
///
/// Marker trait for testing; no actual unsafe semantics.
#[allow(dead_code)] // referenced via batch_trait!; the compiler does not see the impl
unsafe trait YieldUnsafe {}

#[test]
fn batch_trait_multi_segment_unsafe() {
    fn check_pair<T: PairSegment>(_: &T) {}
    check_pair(&0usize);
    check_pair(&0isize);
}

// ============================================================
// 18. batch_impl_only does not emit the trait definition
// ============================================================
trait DropDefOnly {
    fn m(&self) -> u32;
}

#[batch_impl_only(usize #m{42})]
trait DropDefOnly {
    fn m(&self) -> u32;
}

#[test]
fn batch_impl_only_drops_trait() {
    assert_eq!(0usize.m(), 42);
}

// ============================================================
// 19. - operator (left-associative)
// ============================================================
#[batch_impl(HashMap-u32-String)]
trait DashMapGen {}

#[test]
fn dash_op() {
    fn check<T: DashMapGen>(_: &T) {}
    check(&HashMap::<u32, String>::new());
}

// ============================================================
// 20. Nested generic merging <T> Describe<T> [Vec<T>, <U> HashMap<T, U>]
// ============================================================
#[batch_impl(<T> Describe<T> [Vec<T>, <U> HashMap<T, U>] {
    fn describe(&self) -> String { format!("len={}", self.len()) }
})]
trait Describe<T> {
    fn describe(&self) -> String;
}

#[test]
fn nested_generic_list() {
    let v: Vec<i32> = vec![1, 2, 3];
    assert_eq!(v.describe(), "len=3");
    let m: HashMap<i32, String> = HashMap::from([(1, String::from("a"))]);
    assert_eq!(m.describe(), "len=1");
}

// ============================================================
// 21. `where{...}` DSL suffix
// ============================================================
#[batch_impl(<T: Clone> Sortable<T> Vec<T> where{ T: Ord } {
    fn is_sorted(&self) -> bool {
        self.windows(2).all(|w| w[0] <= w[1])
    }
})]
trait Sortable<T> {
    fn is_sorted(&self) -> bool;
}

#[test]
fn dsl_where_clause() {
    let v: Vec<i32> = vec![1, 2, 3];
    assert!(v.is_sorted());
    let v: Vec<i32> = vec![3, 1, 2];
    assert!(!v.is_sorted());
}

// ============================================================
// 22. `where{...}` suffix form (postfix)
// ============================================================
#[batch_impl(
    <T> Singleton<T> Vec<T> where{ T: Clone + Default }
    { fn only(&self) -> T { self.first().cloned().unwrap_or_default() } }
)]
trait Singleton<T> {
    fn only(&self) -> T;
}

#[test]
fn suffix_where_clause() {
    let v: Vec<i32> = vec![42];
    assert_eq!(v.only(), 42);
    let v: Vec<String> = vec![];
    assert_eq!(v.only(), String::new());
}

// ============================================================
// 23. `<A><B>T` merging (apply chain merged into impl<A, B>)
// ============================================================
trait PairAB<A, B> {
    fn pair(&self) -> (A, B);
}

#[batch_impl_only(
    <A> <B> PairAB<A, B> (A, B) where{ A: Clone, B: Clone }
    { fn pair(&self) -> (A, B) { (self.0.clone(), self.1.clone()) } }
)]
trait PairAB<A, B> {
    fn pair(&self) -> (A, B);
}

#[test]
fn nested_generics_merge() {
    let p = (1u32, String::from("x"));
    assert_eq!(p.pair(), (1u32, String::from("x")));
}

// ============================================================
// 24. List modifier + `where{...}` (where attached to the outer Array)
// ============================================================
#[batch_impl(
    <T> WrapOrd<T> [Box, Rc]^Vec<T> where{ T: Ord }
    { fn is_sorted(&self) -> bool { self.windows(2).all(|w| w[0] <= w[1]) } }
)]
trait WrapOrd<T> {
    fn is_sorted(&self) -> bool;
}

#[test]
fn where_with_list_modifier() {
    use std::rc::Rc;
    assert!(WrapOrd::<i32>::is_sorted(&Box::new(vec![1, 2, 3])));
    assert!(!WrapOrd::<i32>::is_sorted(&Rc::new(vec![3, 1, 2])));
}

// ============================================================
// 25. Bare `where predicate {body}` (new syntax; comma predicates are not split by specs)
// ============================================================
#[batch_impl(
    <A> <B> PairComma<A, B> (A, B)
    where A: Clone, B: Clone #both{ (self.0.clone(), self.1.clone()) }
)]
trait PairComma<A, B> {
    fn both(&self) -> (A, B);
}

#[test]
fn where_bare_comma_predicates() {
    let p = (1u32, String::from("x"));
    assert_eq!(PairComma::both(&p), (1u32, String::from("x")));
}

// ============================================================
// 26. Bare where + `m!{}` macro body (a macro invocation is not a body boundary)
// ============================================================
macro_rules! m {
    () => {
        u32
    };
}

#[batch_impl(
    <T> FnRet<T> Vec<T> where T: Fn(u32) -> m!{}
    { fn ret_is_ok(&self) -> bool { true } }
)]
trait FnRet<T> {
    fn ret_is_ok(&self) -> bool;
}

#[test]
fn where_macro_body_excluded() {
    let v: Vec<fn(u32) -> u32> = vec![|x| x + 1];
    assert!(v.ret_is_ok());
}

// ============================================================
// 27. Bare where with multiple segments (`where A where B`) + empty code block
// ============================================================
#[batch_impl(
    <T> MultiOrd<T> Vec<T> where T: Ord where T: Clone {}
)]
trait MultiOrd<T> {}

#[test]
fn where_bare_multi_clause() {
    fn check<T: MultiOrd<i32>>() {}
    check::<Vec<i32>>();
}

// ============================================================
// 28. Open extension mechanism: user macros expand to items based on the trait
//     `usize #batch_preprocess_test(add,inc){*self+1}` expands to a top-level
//     macro call `{ ! batch_preprocess_test!{(add,inc){*self+1} trait AddInc {...}} }`
//     — the `!` marks top-level emission: the spec body `{usize}` is prepended
//     to the macro input (4 segments) and the macro emits its own impl.
//     The manual in-impl form `T {m!{...}}` (no `!`) keeps the call in the
//     impl body (associated items); `T {! m!{...}}` is the top-level form
//     with a user-written input. batch_preprocess_test! parses
//     method names/body/trait and generates fn definitions (or a full impl
//     in the top-level form), equivalent to handing `#fill` to a user macro
//     (each type can carry its own; the trait is not duplicated)
// ============================================================
#[batch_impl(usize #batch_preprocess_test(add,inc){*self+1})]
trait AddInc {
    fn add(&self) -> Self;
    fn inc(&self) -> Self;
}

// Top-level form with a user-written macro call (`{! ...}` attach): the spec
// body `{T}` is prepended — the macro receives 4 segments and emits its own
// impl, same as the `#cmd` form.
#[batch_impl(u16 {! batch_preprocess_test!{(add,inc){*self+3} trait AddIncU16 { fn add(&self) -> Self; fn inc(&self) -> Self; }} })]
trait AddIncU16 {
    fn add(&self) -> Self;
    fn inc(&self) -> Self;
}

// Manual in-impl form: the macro call lands in the impl body (no `!`) — the
// user writes the full input including the trait; the macro emits associated
// fn definitions.
#[batch_impl(u8 { batch_preprocess_test!{(add,inc){*self+2} trait AddIncU8 { fn add(&self) -> Self; fn inc(&self) -> Self; }} })]
trait AddIncU8 {
    fn add(&self) -> Self;
    fn inc(&self) -> Self;
}

#[test]
fn open_extension_fn_like_macro() {
    assert_eq!(5usize.add(), 6);
    assert_eq!(5usize.inc(), 6);
    assert_eq!(5u16.add(), 8);
    assert_eq!(5u16.inc(), 8);
    assert_eq!(5u8.add(), 7);
    assert_eq!(5u8.inc(), 7);
}

// ============================================================
// 29. `unsafe fn(...)` types: `unsafe` modifies the fn type itself
//     (distinct from the unsafe impl marker `unsafe^T`; `unsafe X` errors when X is not a fn)
// ============================================================
#[batch_impl(unsafe fn(u32) -> u32)]
trait UnsafeFnMarker {}

#[batch_impl(unsafe fn^(u32, i32))]
trait UnsafeFnPow {}

#[batch_impl(unsafe fn^(u32, i32) - i64)]
trait UnsafeFnRet {}

#[test]
fn unsafe_fn_type() {
    fn check<T: UnsafeFnMarker>(_: &T) {}
    let f: unsafe fn(u32) -> u32 = |x| x;
    check(&f);

    fn check_pow<T: UnsafeFnPow>(_: &T) {}
    let g: unsafe fn(u32, i32) = |_, _| {};
    check_pow(&g);

    fn check_ret<T: UnsafeFnRet>(_: &T) {}
    let h: unsafe fn(u32, i32) -> i64 = |a, b| a as i64 + b as i64;
    check_ret(&h);
}

// ============================================================
// 30. Directive argument list subtraction: `-name` / `-@all` exclusions (replacing `#except`)
//     (excluded items use the trait's default implementation, verifying they were not batch-generated)
// ============================================================
#[batch_impl(usize #fill(@all,-skip_me){0})]
trait ExceptInline {
    fn keep_me(&self) -> u32;
    fn skip_me(&self) -> u32 {
        999
    }
    const VALUE: u32;
}

// Marker subtraction: @all - @all_methods = const + type
#[batch_impl(isize #fill(@all,-@all_methods){1})]
trait MarkMinus {
    fn m(&self) -> u32 {
        7
    }
    const C: u32;
}

// Explicit list + exclusions
#[batch_impl(u32 #fill(a, -b){2})]
trait ListMinus {
    fn a(&self) -> u32;
    fn b(&self) -> u32 {
        8
    }
}

#[test]
fn directive_minus_exclude() {
    assert_eq!(1usize.keep_me(), 0);
    assert_eq!(1usize.skip_me(), 999);
    assert_eq!(<usize as ExceptInline>::VALUE, 0);

    // `@all - @all_methods` = const + type: methods use their default implementations
    assert_eq!(<isize as MarkMinus>::C, 1);
    assert_eq!(0isize.m(), 7);

    let u = 3u32;
    assert_eq!(u.a(), 2);
    assert_eq!(u.b(), 8);
}

// ============================================================
// 30b. `@all_default*` / `@all_required*`: filter items by default-implementation status
//     (the trait item `default` field: fn=default body, const=default value, type=default type;
//      required ∪ default = all, a closed dichotomy)
// ============================================================
// Combined: required filled with 1, default overridden to 2 (u32)
#[batch_impl(u32 #fill(@all_required_methods){1} #fill(@all_default_methods){2})]
trait ReqDefMix {
    fn req(&self) -> u32;
    fn opt(&self) -> u32 {
        100
    }
}

// Required only: default methods keep the trait's default impl (most common usage, u64)
#[batch_impl(u64 #fill(@all_required_methods){3})]
trait ReqOnly {
    fn req(&self) -> u32;
    fn opt(&self) -> u32 {
        7
    }
}

// blanket + required: only delegate mandatory items; default methods keep the trait default (u16)
#[batch_impl(#blanket(@all_required_methods){Box})]
trait BlanketReq {
    fn req(&self) -> u32;
    fn opt(&self) -> u32 {
        7
    }
}

impl BlanketReq for u16 {
    fn req(&self) -> u32 {
        1
    }
}

#[test]
fn all_default_required_markers() {
    assert_eq!(0u32.req(), 1);
    assert_eq!(0u32.opt(), 2); // default 100 overridden → 2
    assert_eq!(0u64.req(), 3);
    assert_eq!(0u64.opt(), 7); // default kept
    let b = Box::new(1u16);
    assert_eq!(b.req(), 1); // delegates to (*self).req()
    assert_eq!(b.opt(), 7); // default kept
}

// ============================================================
// 31. Empty operand strictness: legal forms are unaffected
//     (trailing commas / empty tuple `()` / empty base `[]` are real tokens, not empty operands)
// ============================================================
// Trailing commas must be guarded with #[rustfmt::skip]: rustfmt removes trailing commas
// from single-line macro invocations, so this case is the regression vehicle for
// "trailing commas are legal"
#[rustfmt::skip]
#[batch_impl(usize, isize,)]
trait TrailingCommaOk {}

#[batch_impl(())]
trait EmptyTupleOk {}

#[batch_impl(usize, isize)]
trait NoTrailingIssue {}

#[test]
fn strictness_legal_forms() {
    fn check<T: TrailingCommaOk>() {}
    check::<usize>();
    check::<isize>();

    fn check2<T: EmptyTupleOk>() {}
    check2::<()>();

    fn check3<T: NoTrailingIssue>() {}
    check3::<isize>();
}

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

// ============================================================
// 35. @ constant system: built-in name families / range families / batch_trait! custom
// ============================================================
#[batch_impl(@u8..u128)]
trait UintConst {}

#[batch_impl(@scalar)]
trait ScalarConst {}

#[batch_impl(@num)]
trait NumConst {}

trait ConstA {}
trait ConstB {}
batch_trait!(
    @nums=[u8, u16, u32];
    @uints=@u*;
    ConstA: @nums;
    ConstB: [Box, Rc]^@uints;
);

#[test]
fn const_system() {
    fn _u<T: UintConst>(_: &T) {}
    _u(&0u8);
    _u(&0u64);
    _u(&0u128);

    fn _s<T: ScalarConst>(_: &T) {}
    _s(&true);
    _s(&'a');
    _s(&0f64);
    _s(&0usize);
    _s(&0i16);

    fn _n<T: NumConst>(_: &T) {}
    _n(&0f32);
    _n(&0i128);

    fn _a<T: ConstA>(_: &T) {}
    _a(&0u8);
    _a(&0u32);
    fn _b<T: ConstB>(_: &T) {}
    _b(&Box::new(0u8));
    _b(&Rc::new(0usize));
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

// Nested wrapping and `:N` depth annotations: `Box^Rc:2` → `Box<Rc<T>>` (delegates `***self`),
// `Box^Box^Box:3` → `Box<Box<Box<T>>>` (delegates `****self`)
#[batch_impl(u32 { fn deep(&self) -> u32 { *self } })]
#[batch_impl(#blanket(deep){Box^Rc:2, Box^Box^Box:3})]
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
    @lazy_wrapped=[Box, Rc]^@lazy_nums;
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

// ============================================================
// 38. Review additions: lazy-expansion value forms + full blanket generic trait forms
//     (values embedding range-family references / bare list values / lists embedding
//     references; multi-type params / const generics / lifetime traits; &mut delegation;
//     non-generic assoc full delegation)
// ============================================================

// Value embedding a range-family reference: `@rv=@u8..u128;` (check_value_refs endpoint
// detection uses split_range_endpoint — the bare name `@u8` is not in the built-in name
// families); definition segments must all precede the trait segments (leading syntax; the
// collection loop stops at the first non-definition segment)
trait RangeVal {}
trait RangeValNested {}
trait BareVal {}
batch_trait!(
    @rv=@u8..u128;
    @nested=[bool, @rv];
    @bare=u8, u32;
    RangeVal: @rv;
    RangeValNested: @nested;
    BareVal: @bare;
);

#[test]
fn lazy_value_forms() {
    fn _r<T: RangeVal>() {}
    _r::<u8>();
    _r::<u64>();
    _r::<u128>();

    fn _n<T: RangeValNested>() {}
    _n::<bool>();
    _n::<u16>();

    fn _b<T: BareVal>() {}
    _b::<u8>();
    _b::<u32>();
}

// blanket generic trait: two type params (the args of the bound `T: Two<A, B>` are grouped
// into an angle-bracket group — 0.6.1 fix: flat `<A, B>` used to be wrongly cut by the
// depth-0 comma split, only correct by render-idempotence luck; this case locks in correct
// parsing after grouping).
// Note: the `#pair` directive copies the trait signature verbatim (A/B are parameter names);
// direct impls must write concrete argument signatures by hand (no parameter substitution);
// a generic `impl<A, B> for (A, B)` would conflict with section 23's PairAB `.pair()` method
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

// ============================================================
// 32. batch_trait! custom @ constant values containing <...> (`@` pairs before `<>`)
//     (0.6.1 fixed the pipeline order `@ <> # where`: previously the @inner of `Vec<@inner>`
//     was paired into the <> group and expand_consts did not enter the group, leaving it
//     behind — an observed compile error)
// ============================================================
trait FooMap {}
trait FooNest {}

batch_trait!(
    @map = HashMap<u32, String>;
    FooMap: @map
);

// Nested: @inner's value contains <...>, @outer references @inner — lazy expansion recursion
batch_trait!(
    @inner = Vec<u8>;
    @outer = Vec<@inner>;
    FooNest: @outer
);

#[test]
fn trait_const_value_with_angles() {
    fn _check_map<T: FooMap>() {}
    fn _check_nest<T: FooNest>() {}
    _check_map::<HashMap<u32, String>>();
    _check_nest::<Vec<Vec<u8>>>();
}

// ============================================================
// 33. Macro meta-layer completion: @trait / @Cow / blanket wrapper where / [a,b] args / where style
// ============================================================
use std::borrow::Cow;

// @trait: batch_impl expands the local trait name (referenced in blanket wrapper where predicates)
#[batch_impl(#blanket(@all_methods){Cow<'_> where{@0: ToOwned + ?Sized, @0::Owned: @trait}})]
trait CowWhereTrait {
    fn klen(&self) -> usize;
}
impl CowWhereTrait for str {
    fn klen(&self) -> usize {
        self.len()
    }
}
impl CowWhereTrait for String {
    fn klen(&self) -> usize {
        self.len()
    }
}

// @Cow: built-in constant (Cow<'_> + inherent constraints, deref target = T::Owned)
#[batch_impl(#blanket(@all_methods){@Cow})]
trait CowConstTrait {
    fn clen(&self) -> usize;
}
impl CowConstTrait for str {
    fn clen(&self) -> usize {
        self.len()
    }
}
impl CowConstTrait for String {
    fn clen(&self) -> usize {
        self.len()
    }
}

// [a,b] hand-written directive args + @all subtraction -[a,b] exclusion
#[batch_impl(u8 #fill([m1, m2]){1} #fill(@all, -[m1, m2]){3})]
trait BracketArgs {
    fn m1(&self) -> u32;
    fn m2(&self) -> u32;
    fn m3(&self) -> u32;
}

// where style: <> keeps only the names, constraints go in where
#[batch_impl(<T> WhereStyle<T> Vec<T> where{T: Clone} { fn wdup(&self) -> usize { self.len() } })]
trait WhereStyle<T: Clone> {
    fn wdup(&self) -> usize;
}

#[test]
fn macro_meta_complete() {
    let c: Cow<'static, str> = Cow::Borrowed("abc");
    assert_eq!(c.klen(), 3); // @trait predicate (@0::Owned: @trait → T::Owned: CowWhereTrait)
    assert_eq!(c.clen(), 3); // @Cow built-in
    let s: Cow<'static, str> = Cow::Owned("xy".to_string());
    assert_eq!(s.klen(), 2);
    assert_eq!(s.clen(), 2);
    assert_eq!(0u8.m1(), 1); // [m1, m2] filled with 1
    assert_eq!(0u8.m2(), 1);
    assert_eq!(0u8.m3(), 3); // @all -[m1, m2] → m3 filled with 3
    let v = vec![1u32];
    assert_eq!(v.wdup(), 1); // where style
}

// @0 generalization: positional references usable in ordinary where predicates
// (tuple-generated generics / user generics)
#[batch_impl(()^2 where{@0: Clone, @1: Copy} { fn tmk() -> u32 { 2 } })]
trait TupleWhereAt {
    fn tmk() -> u32;
}

#[batch_impl(<T> AtWhere<T> Vec<T> where{T: Default} { fn an(&self) -> usize { self.len() } })]
trait AtWhere<T: Clone> {
    fn an(&self) -> usize;
}

// @N is matched by number (= generation order = the target type's document
// order), not by declaration order: in a join the declaration order differs
// from the document order (`()^3-()^3` declares the nested tuple first), so
// `@0` must still be the first fresh as it appears in the target type. The
// `JoinMarker` bound (implemented only for u8) verifies `@0` is the document-
// order first fresh (u8) — the old declaration-order indexing would resolve
// `@0` to the nested tuple's first element (u64) and fail to compile.
trait JoinMarker {}
impl JoinMarker for u8 {}

#[batch_impl(()^3-()^3 where{@0: JoinMarker, @5: Copy})]
trait JoinAtNum {}

#[test]
fn at_refs_numbered_match_in_join() {
    // Trigger instantiation of the generated impl (its where clause checks
    // `_Param_0_: JoinMarker` and `_Param_5_: Copy` against the concrete type).
    fn assert_impl<T: JoinAtNum>() {}
    assert_impl::<(u8, u16, u32, (u64, u128, usize))>();
}

// @all_fresh: every fresh generic gets the predicate tail (comma-separated)
#[batch_impl(()^2-()^2 where{@all_fresh: Clone})]
trait AllFreshWhere {}

// @N..=M: contiguous fresh range — `@0..=1` bounds the first two freshes
#[batch_impl(()^2-()^2 where{@0..=1: Copy})]
trait RangeWhere {}

#[test]
fn at_all_fresh_and_range() {
    // `()^2-()^2` targets `(A, B, (C, D))` (left tuple flattened, right
    // nested). @all_fresh: all 4 fresh generics (swept 0..4) must be Clone.
    fn assert_impl_all<T: AllFreshWhere>() {}
    assert_impl_all::<(u8, u16, (u32, u64))>();
    // @0..=1: only the first two freshes (swept `_Param_0_`, `_Param_1_` —
    // A and B) must be Copy; C, D are unconstrained (String / Vec are not
    // Copy — if the range leaked past `=1` this would fail to compile)
    fn assert_impl_range<T: RangeWhere>() {}
    assert_impl_range::<(u8, u16, (String, Vec<u8>))>();
}

// @all_fresh and @N..=M in the *same* where group: the group is split into
// predicates at depth-0 commas so the @all_fresh expansion must not swallow
// the following @N..=M predicate.
#[batch_impl(()^3-()^3 where{@all_fresh: Clone, @0..=2: Copy})]
trait CombinedBatchWhere {}

#[test]
fn at_all_fresh_with_range_same_group() {
    fn assert_impl<T: CombinedBatchWhere>() {}
    // all 6 freshes Clone + first 3 Copy (u128/usize are Clone; not Copy-bound)
    assert_impl::<(u8, u16, u32, (u64, u128, usize))>();
}

// Fresh names are swept per impl to `_Param_0..N_BatchGen_` (grouped
// `_Param_{g}_{i}_` generation → document-order renumber), so `@N` is a pure
// construction that works across generation units: a range spec generates one
// impl per length, each sweeping its own fresh to 0..N — `@0` is "this impl's
// first" in every length (previously the numbering drifted across lengths and
// `@0` errored on the later impls).
#[batch_impl(()^1..=3 where{@0: Clone} { fn tmk() -> u32 { 2 } })]
trait RangeAtNum {
    fn tmk() -> u32;
}

// Same for multiple specs: each spec sweeps independently, so spec 2's `@0`
// is spec 2's first fresh (previously the counter continued across specs).
#[batch_impl(()^2, ()^3 where{@0: Clone})]
trait MultiAtNum {}

#[test]
fn at_refs_across_generation_units() {
    assert_eq!(<(u8,) as RangeAtNum>::tmk(), 2);
    assert_eq!(<(u8, u16) as RangeAtNum>::tmk(), 2);
    assert_eq!(<(u8, u16, u32) as RangeAtNum>::tmk(), 2);
    // `@0: Clone` must resolve in *both* specs (each sweeps its own fresh).
    fn assert_impl<T: MultiAtNum>() {}
    assert_impl::<(u8, u16)>();
    assert_impl::<(u8, u16, u32)>();
}

// `@g_i` structured references: group g, position i of the generating site.
// In `()^3-()^3` the left generator is group 0, the right is group 1 — `@0_0`
// is the left group's first fresh (A = u8: JoinMarker) and `@1_0` the right
// group's first (D = u64: Copy). Unlike `@N`, `@g_i` is stable across
// array-dispatch impls (a group absent from an impl errors instead of
// silently shifting).
#[batch_impl(()^3-()^3 where{@0_0: JoinMarker, @1_0: Copy})]
trait JoinAtGroup {}

#[test]
fn at_group_position_refs() {
    fn assert_impl<T: JoinAtGroup>() {}
    assert_impl::<(u8, u16, u32, (u64, u128, usize))>();
}

#[test]
fn where_position_refs() {
    assert_eq!(<(u32, u32) as TupleWhereAt>::tmk(), 2);
    let v = vec![1u32];
    assert_eq!(v.an(), 1);
}

// ============================================================
// 34. batch_trait! segment-level @trait: reusing a "generic declaration + trait name" bundle
//     across segments (@trait inside constant values is replaced per segment with that
//     segment's trait path after entry splitting)
// ============================================================
trait SegA<T> {}
trait SegB<T> {}

batch_trait! {
    @type_t = <T> @trait <T>;
    SegA: @type_t [&, Box]^T;
    SegB: @type_t Box^[T, Vec<T>];
}

#[test]
fn trait_const_segment() {
    fn check_a<T: SegA<u8>>() {}
    fn check_b<T: SegB<u8>>() {}
    check_a::<&u8>();
    check_a::<Box<u8>>();
    check_b::<Box<u8>>();
    check_b::<Box<Vec<u8>>>();
}

// ============================================================
// 35. Review additions: all @all status-marker kinds / marker subtraction / @trait top-level
//     spec / [a,b] delegate args / blanket wrapper where @0 / multi-arg tuple @N
// ============================================================

// @all_required (fn + const kinds) fills only required items; defaults are kept
#[batch_impl(u32 #fill(@all_required){4})]
trait ReqMix2 {
    fn rfn(&self) -> u32;
    fn dfn(&self) -> u32 {
        1
    }
    const RC: u32;
    const DC: u32 = 2;
}

// @all_default_constants: only overrides consts with default values (methods excluded)
#[batch_impl(u64 #fill(@all_default_constants){8})]
trait DefConstOnly {
    fn m(&self) -> u32 {
        3
    }
    const C: u32 = 7;
}

// @all_required_types: only fills required types (trait associated type defaults are a
// nightly feature E0658, so `@all_default_types` is unavailable on stable — const/fn
// defaults are stable)
#[batch_impl(u16 #fill(@all_required_types){u16})]
trait ReqTypesOnly {
    type RT;
}

// Marker subtraction: @all_methods - @all_default_methods = required methods only
#[batch_impl(u8 #fill(@all_methods, -@all_default_methods){1})]
trait MarkerMinus2 {
    fn r1(&self) -> u32;
    fn r2(&self) -> u32;
    fn d1(&self) -> u32 {
        9
    }
}

// @trait top-level expansion: the spec's trait-name part is written as `@trait<T>`
// (lazy expansion consumes 2 tokens; the remaining `<T>` is paired by angle_collect)
#[batch_impl(<T> @trait<T> Vec<T> { fn tl(&self) -> usize { self.len() } })]
trait AtTraitSpec<T> {
    fn tl(&self) -> usize;
}

// [a,b] args in #delegate: Box<Vec<u32>> delegates dl1/dl2
#[batch_impl(
    Vec<u32> {
        fn dl1(&self) -> usize { self.len() }
        fn dl2(&self) -> usize { self.len() }
    },
    Box^Vec^u32 #delegate([dl1, dl2]){**self}
)]
trait DelBr {
    fn dl1(&self) -> usize;
    fn dl2(&self) -> usize;
}

// blanket wrapper where with only @0 (no @trait): `Box where{@0: Copy}`
#[batch_impl(u32 { fn own(&self) -> u32 { *self } })]
#[batch_impl(#blanket(own){Box where{@0: Copy}})]
trait OwnAt0 {
    fn own(&self) -> u32;
}

// @N positional reference: ()^3 where{@2: Clone} (fresh generic in the third slot)
#[batch_impl(()^3 where{@2: Clone} { fn tk3() -> u32 { 3 } })]
trait TupleWhereAt3 {
    fn tk3() -> u32;
}

#[test]
fn macro_meta_review_extras() {
    assert_eq!(0u32.rfn(), 4);
    assert_eq!(0u32.dfn(), 1); // default kept
    assert_eq!(<u32 as ReqMix2>::RC, 4);
    assert_eq!(<u32 as ReqMix2>::DC, 2); // default kept

    assert_eq!(<u64 as DefConstOnly>::C, 8); // default const overridden
    assert_eq!(0u64.m(), 3); // methods excluded

    fn _check_t<T: ReqTypesOnly>() {}
    _check_t::<u16>();
    let _: <u16 as ReqTypesOnly>::RT = 5u16;

    assert_eq!(0u8.r1(), 1);
    assert_eq!(0u8.r2(), 1);
    assert_eq!(0u8.d1(), 9); // default method kept

    let v = vec![1u32, 2];
    assert_eq!(v.tl(), 2);

    let b: Box<Vec<u32>> = Box::new(vec![1, 2, 3]);
    assert_eq!(b.dl1(), 3);
    assert_eq!(b.dl2(), 3);

    assert_eq!(Box::new(5u32).own(), 5);
    assert_eq!(<(u8, u16, u32) as TupleWhereAt3>::tk3(), 3);
}

// ============================================================
// 36. Review fix lock: B1 (codegen @trait case-sensitivity) + B2 (@ inside None groups from macro variables)
// ============================================================
// B1: @trait in ordinary where predicates (codegen resolve_where_at path —
// previously compared id == "Trait" with a capital, wrongly rejecting @trait)
#[batch_impl(<T> WhereAtTrait<T> Vec<T> where{T: @trait<T>} { fn wn(&self) -> usize { self.len() } })]
trait WhereAtTrait<T: Clone> {
    fn wn(&self) -> usize;
}
impl WhereAtTrait<u32> for u32 {
    fn wn(&self) -> usize {
        1
    }
}

// B2: macro-variable expansion produces real None groups ($($spec)* repeated expansion);
// @u* inside groups must expand
macro_rules! make_impls {
    ($($spec:tt)*) => {
        #[batch_impl($($spec)*)]
        trait MacroGenTrait {
            fn gm(&self) -> u32;
        }
    };
}
make_impls!([Box, Rc]^@u* { fn gm(&self) -> u32 { 9 } });

#[test]
fn review_fixes_locked() {
    let v = vec![1u32];
    assert_eq!(v.wn(), 1); // B1: @trait expands correctly in ordinary where
    let b = Box::new(1u32);
    let r = Rc::new(1u32);
    assert_eq!(b.gm(), 9); // B2: @u* expands inside macro-variable None groups
    assert_eq!(r.gm(), 9);
}

// ============================================================
// Receiver-kind `@all` filters (`@all_ref_methods` / `@all_value_methods` / `@all_static_methods`)
// ============================================================

#[test]
fn receiver_kind_filters() {
    #[batch_impl(
        u8
        #fill(@all_ref_methods){ 7 }
        #fill(@all_value_methods){ 8 }
        #fill(@all_static_methods){ 9 }
        #C{ 10 }
        #Item{ u8 }
    )]
    trait RecvT {
        fn by_ref(&self) -> u8;
        fn by_mut(&mut self) -> u8;
        fn by_val(self) -> u8;
        fn make() -> u8;
        const C: u8;
        type Item;
    }

    let x = 5u8;
    assert_eq!(RecvT::by_ref(&x), 7);
    let mut y = 5u8;
    assert_eq!(RecvT::by_mut(&mut y), 7);
    assert_eq!(RecvT::by_val(x), 8);
    assert_eq!(<u8 as RecvT>::make(), 9);
    assert_eq!(<u8 as RecvT>::C, 10);
    let _: <u8 as RecvT>::Item = 1u8;
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

// ============================================================
// Reviewer additions: typed-receiver filter + marker-minus-marker
// ============================================================

// `@all_value_methods` includes typed receivers (`self: Box<Self>`,
// `syn::ReceiverKind::Typed`); `@all_static_methods` = no receiver.
#[batch_impl(u8 #fill(@all_value_methods){4} #fill(@all_static_methods){5})]
trait TypedRecv2 {
    fn plain(self) -> u8;
    fn boxed(self: Box<Self>) -> u8;
    fn by_ref(&self) -> u8 {
        7
    }
    fn make() -> u8;
}

// Marker-minus-marker: `@all_methods - @all_value_methods` = ref + static.
// (minus takes a resolved `[...]` list; `-@all_value_methods` expands to it)
#[batch_impl(u16 #fill(@all_methods, -@all_value_methods){6})]
trait MarkerMinus3 {
    fn by_ref(&self) -> u16;
    fn by_val(self) -> u16
    where
        Self: Sized,
    {
        0
    }
    fn make() -> u16;
}

#[test]
fn receiver_filters_review() {
    assert_eq!(TypedRecv2::plain(3u8), 4);
    assert_eq!(TypedRecv2::boxed(Box::new(3u8)), 4);
    assert_eq!(TypedRecv2::by_ref(&3u8), 7); // excluded -> default
    assert_eq!(<u8 as TypedRecv2>::make(), 5);

    assert_eq!(MarkerMinus3::by_ref(&1u16), 6);
    assert_eq!(MarkerMinus3::by_val(1u16), 0); // excluded -> default
    assert_eq!(<u16 as MarkerMinus3>::make(), 6);
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
#[batch_impl(@all_type_params GenT<T> Vec<T> { fn head(&self) -> T { self[0].clone() } })]
trait GenT<T: Clone> {
    fn head(&self) -> T;
}

#[batch_impl(@all_lifetimes @all_type_params GenB<'a, T> &'a T { fn get(&self) -> &'a T { self } })]
trait GenB<'a, T: Clone> {
    fn get(&self) -> &'a T;
}

#[batch_impl(@all_const_params GenC<N> [u8; N] { fn n(&self) -> usize { N } })]
trait GenC<const N: usize> {
    fn n(&self) -> usize;
}

#[test]
fn generic_param_families() {
    // type params: declaration auto-copied, bounds via same-name inheritance
    let v = vec![1u32, 2];
    assert_eq!(v.head(), 1u32);
    // lifetime + type combination (consecutive declarations keep lifetimes first)
    let b = &5u8;
    assert_eq!(b.get(), &5u8);
    // const params: full `const N: usize` declaration
    let a: [u8; 3] = [1, 2, 3];
    assert_eq!(a.n(), 3);
}
#[batch_impl((()^3,)^3)]
trait NestedGenT {}

#[test]
fn nested_generator_in_tuple_pow() {
    // (T,)^3 clones the generator's fresh declarations; hoisting must
    // dedupe them so the impl has one shared generic trio.
    fn assert_trait<T: NestedGenT>() {}
    assert_trait::<((u8, u16, u32), (u8, u16, u32), (u8, u16, u32))>();
}
// ---- List distribution in nested positions (0.7.0) ----

#[batch_impl((u8, [u16, u32, u64]))]
trait TupDist {}

#[batch_impl(([u8, u16], [u32, u64]))]
trait TupDist2 {}

#[batch_impl(Vec<[u8, u16, u32]>)]
trait GenDist {}

#[batch_impl(Box<[u8, u16]>)]
trait TraitDist {}

#[test]
fn list_distribution_nested() {
    fn assert_t1<T: TupDist>() {}
    assert_t1::<(u8, u16)>();
    assert_t1::<(u8, u64)>();
    fn assert_t2<T: TupDist2>() {}
    assert_t2::<(u8, u32)>();
    assert_t2::<(u16, u64)>();
    fn assert_g<T: GenDist>() {}
    assert_g::<Vec<u8>>();
    assert_g::<Vec<u32>>();
    fn assert_t<T: TraitDist>() {}
    assert_t::<Box<u8>>();
    assert_t::<Box<u16>>();
}

// pow_cartesian output nested in an outer tuple: the array of combos must
// distribute through the tuple (user scenario `(()^2, ((A_,)^2,(<Clone>,)^2)^3, ()^4)^2`).
// Fresh counts differ per generator (`()^2` / `()^3`) so no combo pair
// overlaps after the sweep rename (E0119 is the user's responsibility when
// concrete and fresh generators collide).
struct A_;
#[batch_impl(((A_,)^2, ((A_,)^2,(<Clone>,)^2)^2, ()^3)^2)]
trait NestedPow {}

#[test]
fn pow_cartesian_nested_in_tuple() {
    fn assert_t<T: NestedPow>() {}
    // `[e0, e0]` — both positions pick the `(A_,)^2` generator
    assert_t::<((A_, A_), (A_, A_))>();
    // `[e0, e2]` — position 2 picks the `()^3` fresh trio
    assert_t::<((A_, A_), (u8, u16, u32))>();
    // `[e0, e1_1]` — position 2 picks an inner cartesian combo
    // (`((A_,)^2, (<Clone>,)^2)` combo 2 = `(A_,A_), (C0,C1)`)
    assert_t::<((A_, A_), ((A_, A_), (u8, u16)))>();
}

// ---- Splat (`*` prefix): flatten containers / generators (0.7.0) ----

struct SplatA;
struct SplatB;
struct SplatC;
struct SplatD;
struct SplatE;
struct SplatF;
struct Pair<A, B>(A, B);
struct Triple<A, B, C>(A, B, C);

#[batch_impl([SplatA, *[SplatD, SplatE, SplatF]])]
trait SplatArr {}

#[batch_impl((SplatA, SplatB, SplatC)^*(SplatD, SplatE, SplatF))]
trait SplatConcat {}

#[batch_impl((*(()^3)))]
trait SplatGen {}

#[batch_impl((SplatA, *(()^3)))]
trait SplatGenFlat {}

#[batch_impl(*[Vec, Box]^SplatF)]
trait SplatLeft {}

#[batch_impl(Pair^*(SplatD, SplatE))]
trait SplatArgs {}

#[test]
fn splat_scenarios() {
    fn assert_t<T: SplatArr>() {}
    assert_t::<SplatA>();
    assert_t::<SplatD>();
    assert_t::<SplatF>();
    fn assert_c<T: SplatConcat>() {}
    assert_c::<(SplatA, SplatB, SplatC, SplatD, SplatE, SplatF)>();
    fn assert_g<T: SplatGen>() {}
    assert_g::<(u8, u16, u32)>();
    fn assert_gf<T: SplatGenFlat>() {}
    assert_gf::<(SplatA, u8, u16, u32)>();
    fn assert_l<T: SplatLeft>() {}
    assert_l::<Vec<SplatF>>();
    assert_l::<Box<SplatF>>();
    fn assert_args<T: SplatArgs>() {}
    assert_args::<Pair<SplatD, SplatE>>();
}

// nested splat is idempotent; empty splat is a no-op
#[batch_impl((*(*[SplatD, SplatE])))]
trait SplatNested {}

#[batch_impl([SplatA, *()])]
trait SplatEmpty {}

#[test]
fn splat_idempotent_and_empty() {
    fn assert_n<T: SplatNested>() {}
    assert_n::<(SplatD, SplatE)>();
    fn assert_e<T: SplatEmpty>() {}
    assert_e::<SplatA>();
}

// trailing-comma splat; empty splat in the middle of a tuple
#[batch_impl((*(SplatA,)))]
trait SplatTrailingComma {}

#[batch_impl((SplatA, *(), SplatB))]
trait SplatMiddleEmpty {}

#[test]
fn splat_trailing_comma_and_middle_empty() {
    fn assert_t<T: SplatTrailingComma>() {}
    assert_t::<(SplatA,)>();
    fn assert_m<T: SplatMiddleEmpty>() {}
    assert_m::<(SplatA, SplatB)>();
}

// `[*(a,b)]` — lone splat at the slice position flattens into a list
// (syntax parity with `(*(a,b))` → `(a,b)`).
#[batch_impl([*(SplatA, SplatB)])]
trait SplatLoneArray {}

#[test]
fn splat_lone_array() {
    fn assert_t<T: SplatLoneArray>() {}
    assert_t::<SplatA>();
    assert_t::<SplatB>();
}

// generic-arg splat: `Pair<*(A, B)>` → `Pair<A, B>` (one impl, multi-arg)
// — distinct from `Pair<[A, B]>` which dispatches.
#[batch_impl(Pair<*(SplatA, SplatB)>)]
trait SplatGenArgs {}

#[batch_impl(Pair<*(SplatA, *(SplatB))>)]
trait SplatGenArgsNested {}

#[test]
fn splat_generic_args() {
    fn assert_t<T: SplatGenArgs>() {}
    assert_t::<Pair<SplatA, SplatB>>();
    fn assert_n<T: SplatGenArgsNested>() {}
    assert_n::<Pair<SplatA, SplatB>>();
}

// Splat rules: R1 `T^*(A,B)` ≡ `T-A-B` (right operand always flattens);
// R2 left semantics by source — `*[...]` distributes `^T` (`*[A^T,B^T]`,
// enabling composition `X^*[A,B]^T` = `X<A^T, B^T>`, one impl), `*(...)`
// appends (`*(A,B,...,T)`, list semantics).
#[batch_impl(Pair^*[Vec, Box]^u16)]
trait SplatRule2 {}

#[batch_impl(Pair^*(Vec<u8>, Box<u8>))]
trait SplatRule1 {}

#[batch_impl((SplatA, SplatB)^*(SplatC, SplatD))]
trait SplatConcat2 {}

#[batch_impl(Triple^*(SplatA, SplatB)^SplatC)]
trait SplatParenAppend {}

#[batch_impl(*(SplatA, SplatB)^SplatC)]
trait SplatParenLeft {}

#[batch_impl(*[Vec, Box]^SplatC)]
trait SplatBracketLeft {}

#[test]
fn splat_rules() {
    fn assert_r2<T: SplatRule2>() {}
    assert_r2::<Pair<Vec<u16>, Box<u16>>>();
    fn assert_r1<T: SplatRule1>() {}
    assert_r1::<Pair<Vec<u8>, Box<u8>>>();
    fn assert_c<T: SplatConcat2>() {}
    assert_c::<(SplatA, SplatB, SplatC, SplatD)>();
    // Source-driven left semantics: `*(...)` appends the operand
    // (list — mirrors TyTuple), `*[...]` distributes it (set — mirrors
    // TyArray).
    fn assert_pa<T: SplatParenAppend>() {}
    assert_pa::<Triple<SplatA, SplatB, SplatC>>();
    fn assert_pl<T: SplatParenLeft>() {}
    assert_pl::<SplatA>();
    assert_pl::<SplatB>();
    assert_pl::<SplatC>();
    fn assert_bl<T: SplatBracketLeft>() {}
    assert_bl::<Vec<SplatC>>();
    assert_bl::<Box<SplatC>>();
}

// `*(A,B)^N` — pow Cartesian combos re-wrap into splats:
// `*(A,B)^2` = `[*(A,A), *(A,B), *(B,A), *(B,B)]`. Each combo is a
// param-position list — a right-splat chain flattens it into the container
// (`A^*(A,B)^2` = `A<A,A>`/`A<A,B>`/...; a lone target flattens to
// duplicates, E0119 — use `(A,B)^2` for tuple impls). `*()^N` (empty
// splat) keeps its splat shape so a carrier appends the fresh params into
// it: `T^*()^2` = `<A,B>T<A,B>` (bare `*()^N` lone target → E0207).
#[batch_impl(Pair^*(SplatA, SplatB)^2)]
trait SplatTuplePow {}

#[batch_impl(Pair^*()^2)]
trait SplatEmptyPowCarrier {}

#[test]
fn splat_pow() {
    // `Pair^*(A,B)^2` — the 4 Cartesian combos flatten into Pair's args.
    fn assert_p<T: SplatTuplePow>() {}
    assert_p::<Pair<SplatA, SplatA>>();
    assert_p::<Pair<SplatA, SplatB>>();
    assert_p::<Pair<SplatB, SplatA>>();
    assert_p::<Pair<SplatB, SplatB>>();
    // `Pair^*()^2` emits `impl<P0, P1> SplatEmptyPowCarrier for
    // Pair<P0, P1>` — the carrier consumes the full fresh declaration.
    fn assert_c<T: SplatEmptyPowCarrier>() {}
    assert_c::<Pair<SplatA, SplatB>>();
}

// Splat expands ONE layer: tuples are types and stay intact — `*((a,b),)`
// is one tuple impl, and `*(a,b,(c,d))` keeps `(c,d)` as a single element.
#[batch_impl(*((SplatA, SplatB),))]
trait SplatTupleKeep {}

#[batch_impl(*(SplatA, SplatB, (SplatC, SplatD)))]
trait SplatTupleKeepList {}

#[batch_impl(*(SplatA, SplatB)^(SplatC, SplatD))]
trait SplatGroupRight {}

// The repeat-list shorthand: `Pair^*(*@u*)^2` = `Pair<@u*, @u*>` — one
// `@u*` written once, Cartesian over both param positions.
#[batch_impl(Pair^*(*@u*)^2)]
trait RepeatList {}

#[test]
fn splat_one_layer() {
    fn assert_k<T: SplatTupleKeep>() {}
    assert_k::<(SplatA, SplatB)>();
    fn assert_kl<T: SplatTupleKeepList>() {}
    assert_kl::<SplatA>();
    assert_kl::<SplatB>();
    assert_kl::<(SplatC, SplatD)>();
    // `^(c,d)` (group right) appends the tuple intact — same shape as
    // writing `*(a,b,(c,d))` directly.
    fn assert_gr<T: SplatGroupRight>() {}
    assert_gr::<SplatA>();
    assert_gr::<SplatB>();
    assert_gr::<(SplatC, SplatD)>();
    // `Pair^*(*@u*)^2` — Cartesian over both positions (spot-check a few).
    fn assert_rl<T: RepeatList>() {}
    assert_rl::<Pair<u8, u8>>();
    assert_rl::<Pair<u8, usize>>();
    assert_rl::<Pair<u128, u32>>();
    assert_rl::<Pair<usize, usize>>();
}

// Trait generic args (`Conv<bool>` in the spec): the trait param in copied
// directive signatures is substituted — `fn conv(value: T)` becomes
// `fn conv(value: bool)` (compiling proves the substitution; a raw `T`
// would be E0425). The real trait is defined here; the batch_impl_only
// item below is only a discarded signature source.
trait Conv<T>: Sized {
    fn conv(_value: T) -> Self;
}

#[batch_impl_only(
    Conv<bool>
    [Pair<SplatA, SplatA>, Pair<SplatB, SplatB>]
    #conv{unimplemented!()}
)]
pub trait Conv<T>: Sized {
    fn conv(_value: T) -> Self;
}

#[test]
fn trait_generic_args() {
    // Reference the generated method (proves the impl exists with the
    // substituted signature) — never call it (unimplemented! body).
    fn assert_c<T: Conv<bool>>() {
        let _ = <T as Conv<bool>>::conv;
    }
    assert_c::<Pair<SplatA, SplatA>>();
    assert_c::<Pair<SplatB, SplatB>>();
}
