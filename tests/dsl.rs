// batch-impl DSL 功能性回归测试。
//
// 这些用例覆盖最核心特性：基础批量生成、泛型、并列列表、^/- 操作符、
// 元组生成、unsafe impl、关联类型绑定、fn 类型、属性支持、# 指令系统。
// examples/ 下保留完整的逐项 println 风格用例，本文件提供 `#[test]` 化的核心覆盖。

use batch_impl::{batch_impl, batch_impl_only, batch_preprocess_test, batch_trait};
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
/// # Safety
///
/// 测试用标记 trait，无实际 unsafe 语义。
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
/// # Safety
///
/// 测试用标记 trait，无实际 unsafe 语义。
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

/// # Safety
///
/// 测试用标记 trait，无实际 unsafe 语义。
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
// 21. `where{...}` DSL 后缀
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
fn suffix_where_clause() {
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

// ============================================================
// 24. 列表修饰符 + `where{...}`（where 附着在 Array 外层）
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
// 25. 裸 `where 谓词 {body}`（新语法，逗号谓词不被 spec 切分）
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
// 26. 裸 where + `m!{}` 宏体（宏调用不是 body 边界）
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
// 27. 裸 where 多段（`where A where B`）+ 空代码块
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
// 28. 开放扩展机制：用户宏根据 trait 展开为需要的 fn 定义
//     `usize #batch_preprocess_test(add,inc){*self+1}` 展开为
//     `usize {batch_preprocess_test!{(add,inc){*self+1} trait AddInc {...}}}` —— 宏调用
//     落在 impl body，由 batch_preprocess_test! 解析方法名/body/trait 生成 fn 定义，
//     等价于把 `#fill` 的实现交给用户宏（每类型可各挂一个，trait 不重复）
// ============================================================
#[batch_impl(usize #batch_preprocess_test(add,inc){*self+1})]
trait AddInc {
    fn add(&self) -> Self;
    fn inc(&self) -> Self;
}

#[test]
fn open_extension_fn_like_macro() {
    assert_eq!(5usize.add(), 6);
    assert_eq!(5usize.inc(), 6);
}

// ============================================================
// 29. `unsafe fn(...)` 类型：`unsafe` 修饰 fn 类型本身
//     （区别于 `unsafe^T` 的 unsafe impl 标记；`unsafe X` 非 fn 会报错）
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
// 30. 指令参数列表减法：`-name` / `-#all` 排除项（取代 `#except`）
//     （排除项走 trait 默认实现，验证未被批量生成）
// ============================================================
#[batch_impl(usize #fill(#all,-skip_me){0})]
trait ExceptInline {
    fn keep_me(&self) -> u32;
    fn skip_me(&self) -> u32 {
        999
    }
    const VALUE: u32;
}

// 标记减法：#all - #all_methods = const + type
#[batch_impl(isize #fill(#all,-#all_methods){1})]
trait MarkMinus {
    fn m(&self) -> u32 {
        7
    }
    const C: u32;
}

// 显式列表 + 排除项
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

    // `#all - #all_methods` = const + type：方法走默认实现
    assert_eq!(<isize as MarkMinus>::C, 1);
    assert_eq!(0isize.m(), 7);

    let u = 3u32;
    assert_eq!(u.a(), 2);
    assert_eq!(u.b(), 8);
}

// ============================================================
// 31. 空操作数严格化：合法形态不受影响
//     （尾随逗号 / 空元组 `()` / 空基座 `[]` 都是真实 token，不是空操作数）
// ============================================================
// 尾随逗号必须用 #[rustfmt::skip] 保护：rustfmt 会移除单行宏调用的尾随逗号，
// 这条用例正是"尾随逗号合法"的回归载体
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
// 32. trait 泛型 bound 自动继承：未写 bound 的 impl 泛型参数按位置 + 同名继承
//     （写了 = 用户负责，宏不干预——sub trait 蕴含（`trait B: A` 使 `T: B`
//     隐含 `T: A`）宏无法推理，交由 rustc 验证；异名明确报错，绝不静默）
// ============================================================
#[batch_impl(<T> Cloned<T> Vec<T> {
    fn get(&self) -> T {
        self[0].clone()
    }
})]
trait Cloned<T: Clone> {
    fn get(&self) -> T;
}

// 用户已写 bound（B: A 蕴含 T: A）→ 不干预
trait SupA {}
trait SupB: SupA {}
struct SupS;
impl SupA for SupS {}
impl SupB for SupS {}
#[batch_impl(<T: SupB> Inherit<T> ())]
trait Inherit<T: SupA> {}

// 生命周期 bound 继承：`<'a, T>` → `impl<'a, T: 'a>`
#[batch_impl(<'a, T> Lifetime<'a, T> ())]
trait Lifetime<'a, T: 'a> {}

// 改名场景：生命周期名改（'b vs 'a），trait 名保持一致——impl 无同名 `'a`，
// 生命周期 bound 不继承，用户手写 `T: 'b`
#[batch_impl(<'b, T: 'b> LifetimeRenamed<'b, T> ())]
trait LifetimeRenamed<'a, T: 'a> {}

// `'static` 全局可用：无需声明，照常继承
#[batch_impl(<T> StaticT<T> ())]
trait StaticT<T: 'static> {}

// 混合 bound：Clone + 'a 一并继承
#[batch_impl(<'a, T> Mix<'a, T> ())]
trait Mix<'a, T: Clone + 'a> {}

// 部分绑定：T 用户写 bound（B 蕴含 A，rustc 验证），U 未写（同名继承 A）——
// 继承按参数独立判断，部分写/部分继承天然混合
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

    // 部分绑定：impl<T: SupB, U: SupA> / impl<T: SupA, U: SupB>
    fn check_p<T: PartialBound<SupS, i32>>() {}
    check_p::<()>();
    fn check_p2<T: PartialBound2<i32, SupS>>() {}
    check_p2::<()>();
}

// ============================================================
// 33. `A<>`：trait 泛型照抄——实参与 bound 全部来自 trait 定义，
//     展开为 `<'a, T: bounds, const N> A<'a, T, N>`（与手写等价）
// ============================================================
#[batch_impl(EmptyGenA<> ())]
trait EmptyGenA<T: Clone> {}

#[batch_impl(EmptyGenB<> ())]
trait EmptyGenB<'a, T: 'a> {}

#[batch_impl(EmptyGenC<> Vec<T>)]
trait EmptyGenC<T> {}

// `A<绑定们>`：位置实参照抄 + 关联类型绑定保留
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
// 34. trait 级 where 子句继承：单一形参谓词合并进 bound，其余谓词原样透传
//     （`trait Foo<T> where T: Clone` → `impl<T: Clone>`；
//     `T::Item: Clone` 等复合谓词 → impl 的 where 子句，`<T>` 与 `<>` 同效；
//     引用收集在 syn AST 上做：`A::B` 的 B 是关联类型名，不误判为形参）
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

// where 谓词 + 内联 bound 合并：T: Clone（内联）+ T: Ord（where）
#[batch_impl(<T> WhereBoth<T> ())]
trait WhereBoth<T: Clone>
where
    T: Ord,
{
}

// 生命周期 where 谓词：`T: 'a`
#[batch_impl(<'a, T> WhereLifetime<'a, T> ())]
trait WhereLifetime<'a, T>
where
    T: 'a,
{
}

// 复合谓词 `T::Item: Clone` 原样透传（`<T>` 写法）
#[batch_impl(<T> WhereGen<T> ())]
trait WhereGen<T: Clone>
where
    T: IntoIterator,
    T::Item: Clone,
{
}

// 复合谓词同款（`A<>` 照抄写法）
#[batch_impl(WhereGen2<> ())]
trait WhereGen2<T: Clone>
where
    T: IntoIterator,
    T::Item: Clone,
{
}

// 撞名：`A::B` 的 B 是关联类型名（非形参引用）——impl 只声明 A 也不报错
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
}
