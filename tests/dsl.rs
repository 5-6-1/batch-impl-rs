// batch-impl DSL 功能性回归测试。
//
// 这些用例覆盖最核心特性：基础批量生成、泛型、并列列表、^/- 操作符、
// 元组生成、unsafe impl、关联类型绑定、fn 类型、属性支持、# 指令系统。
// examples/ 下保留完整的逐项 println 风格用例，本文件提供 `#[test]` 化的核心覆盖。

use batch_impl::{batch_impl, batch_impl_only, batch_trait};
use std::collections::HashMap;
use std::rc::Rc;

// ============================================================
// 1. 基础：为具体类型直接实现
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
// 2. 泛型：<T> Vec<T>
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
// 3. 共享 + 独立 body 合并
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
// 4. ^ 操作符：[&, Box, Rc]^u32 笛卡尔积
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
// 5. 元组生成：()^3
// ============================================================
#[batch_impl(()^3)]
trait Tuple3 {}

#[test]
fn tuple_pow_basic() {
    fn check<T: Tuple3>(_: &T) {}
    check(&(1u8, 2u16, 3u32));
}

// ============================================================
// 6. 范围元组：()^1..=3
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
// 7. 关联类型绑定：<T> Iter<Item=T> Vec<T> {...}
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
// 8. unsafe impl：unsafe 前 TRAIT 全 unsafe
// ============================================================
#[batch_impl(usize, Box<u32>)]
unsafe trait UnsafeAll {}

#[test]
fn unsafe_trait_impls() {
    fn check<T: UnsafeAll>(_: &T) {}
    check(&0usize);
    check(&Box::new(0u32));
}

// ============================================================
// 9. 局部 unsafe
// ============================================================
#[batch_impl(unsafe^usize, isize)]
unsafe trait PartialUnsafe {}

#[test]
fn partial_unsafe() {
    fn check<T: PartialUnsafe>(_: &T) {}
    check(&0usize);
    check(&0isize);
}

// ============================================================
// 10. fn 类型
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
// 11. 属性支持：#[allow(dead_code)]^usize
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
// 12. 复杂类型透传
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
// 13. #name{body} 单 item 赋值
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
// 14. #fill(args){body} 多方法同 body
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
// 15. #delegate 委托
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

// ============================================================
// 16. batch_trait! 函数式宏
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
// 17. batch_trait! 多段落 + unsafe 段
// ============================================================
trait PairSegment {}

batch_trait!(
    PairSegment: usize, isize;
    unsafe YieldUnsafe: u32
);

#[allow(dead_code)] // 通过 batch_trait! 引用，编译器看不到 impl
unsafe trait YieldUnsafe {}

#[test]
fn batch_trait_multi_segment_unsafe() {
    fn check_pair<T: PairSegment>(_: &T) {}
    check_pair(&0usize);
    check_pair(&0isize);
}

// ============================================================
// 18. batch_impl_only 不输出 trait 定义
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
// 19. - 操作符（左结合）
// ============================================================
#[batch_impl(HashMap-u32-String)]
trait DashMapGen {}

#[test]
fn dash_op() {
    fn check<T: DashMapGen>(_: &T) {}
    check(&HashMap::<u32, String>::new());
}

// ============================================================
// 20. 嵌套泛型合并 <T> Describe<T> [Vec<T>, <U> HashMap<T, U>]
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
// 21. `where { ... }` DSL 后缀
// ============================================================
#[batch_impl(<T: Clone> Sortable<T> Vec<T> where { T: Ord } {
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
// 22. `where{...}` 后缀形式（后置）
// ============================================================
#[batch_impl(
    <T> Singleton<T> Vec<T> where{ T: Clone + Default }
    { fn only(&self) -> T { self.first().cloned().unwrap_or_default() } }
)]
trait Singleton<T> {
    fn only(&self) -> T;
}

#[test]
fn directive_where_clause() {
    let v: Vec<i32> = vec![42];
    assert_eq!(v.only(), 42);
    let v: Vec<String> = vec![];
    assert_eq!(v.only(), String::new());
}

// ============================================================
// 23. `<A><B>T` 合并（apply chain 合并到 impl<A, B>）
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

