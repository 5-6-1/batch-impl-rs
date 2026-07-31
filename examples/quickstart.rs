// batch-impl 快速上手 demo —— 单文件可运行展示 DSL 主要特性。
//
// 运行：`cargo run --example quickstart`
// 期望输出：每条都打印 `...: OK`，最后打印总览。

use batch_impl::{batch_impl, batch_impl_only, batch_trait};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

// ------------------------------------------------------------
// 1. 基础：为多个具体类型同时实现同一 trait
// ------------------------------------------------------------
#[batch_impl(usize, isize, f32, f64)]
trait Numeric {}

fn demos_basic() {
    fn check<T: Numeric>(_: &T) {}
    check(&0usize);
    check(&0isize);
    check(&0.0f32);
    check(&0.0f64);
    println!("  1. basic (4 concrete types): OK");
}

// ------------------------------------------------------------
// 2. 泛型：`<T> Vec<T>` 一行声明
// ------------------------------------------------------------
#[batch_impl(<T> Vec<T>)]
trait Collection {}

fn demos_generic() {
    fn check<T: Collection>(_: &T) {}
    check(&vec![1, 2, 3]);
    check(&vec!["a", "b"]);
    println!("  2. generic (`<T> Vec<T>`): OK");
}

// ------------------------------------------------------------
// 3. 共享 body + 独立 body 合并
// ------------------------------------------------------------
#[batch_impl(
    [usize { fn name() -> &'static str { "usize" } },
     isize { fn name() -> &'static str { "isize" } }]
    { fn zero() -> Self { 0 } }
)]
trait Zero {
    fn zero() -> Self;
    fn name() -> &'static str;
}

fn demos_shared_independent_body() {
    assert_eq!(usize::zero(), 0);
    assert_eq!(isize::zero(), 0);
    assert_eq!(usize::name(), "usize");
    assert_eq!(isize::name(), "isize");
    println!("  3. shared + independent body merge: OK");
}

// ------------------------------------------------------------
// 4. `^` 运算符：容器 / 引用 / 笛卡尔积
// ------------------------------------------------------------
#[batch_impl([&, Box, Rc, Arc]^u32)]
trait RefOrOwned {}

fn demos_caret() {
    fn check<T: RefOrOwned>(_: &T) {}
    let v: u32 = 7;
    check(&(&v));
    check(&Box::new(v));
    check(&Rc::new(v));
    check(&Arc::new(v));
    println!("  4. `^` cartesian product ([&, Box, Rc, Arc]^u32): OK");
}

// ------------------------------------------------------------
// 5. 元组生成：`()^3` 自动生成泛型元组
// ------------------------------------------------------------
#[batch_impl(()^3)]
trait Tuple {}

#[batch_impl((u8, u16, u32) {
    fn sum(&self) -> u32 { self.0 as u32 + self.1 as u32 + self.2 }
})]
trait TupleSum {
    fn sum(&self) -> u32;
}

fn demos_tuple_gen() {
    fn _check<T: Tuple>() {}
    _check::<(u8, u16, u32)>();
    _check::<(i8, i16, i32)>();
    _check::<(u32, u32, u32)>();
    let t: (u8, u16, u32) = (1, 2, 3);
    assert_eq!(<(u8, u16, u32) as TupleSum>::sum(&t), 6);
    println!("  5. tuple gen + concrete tuple body: OK");
}

// ------------------------------------------------------------
// 6. 关联类型绑定：`<T> Iter<Item=T> Vec<T>`
// ------------------------------------------------------------
#[batch_impl(<T> Iter<Item=T> Vec<T> {
    fn count(&self) -> usize { self.len() }
})]
trait Iter {
    type Item;
    fn count(&self) -> usize;
}

fn demos_assoc_binding() {
    let v: Vec<i32> = vec![1, 2, 3];
    assert_eq!(v.count(), 3);
    println!("  6. associated type binding: OK");
}

// ------------------------------------------------------------
// 7. unsafe 支持：单条标记 + unsafe trait
// ------------------------------------------------------------
/// # Safety
///
/// 演示用标记 trait，无实际 unsafe 语义。
#[batch_impl(unsafe^usize, isize)]
unsafe trait PartialUnsafe {}

/// # Safety
///
/// 演示用标记 trait，无实际 unsafe 语义。
#[batch_impl(u8, u16, u32)]
unsafe trait AllUnsafe {}

fn demos_unsafe() {
    fn check_partial<T: PartialUnsafe>(_: &T) {}
    fn check_all<T: AllUnsafe>(_: &T) {}
    check_partial(&0usize);
    check_partial(&0isize);
    check_all(&0u8);
    check_all(&0u16);
    check_all(&0u32);
    println!("  7. unsafe impl (per-spec + trait-level): OK");
}

// ------------------------------------------------------------
// 8. fn 类型与返回类型
// ------------------------------------------------------------
#[batch_impl(fn^(i32, u32))]
trait FnProbe {}

#[batch_impl(fn(i32, u32)-String)]
trait FnProbeWithRet {}

fn demos_fn_types() {
    fn check<T: FnProbe>(_: &T) {}
    fn check_ret<T: FnProbeWithRet>(_: &T) {}
    let f: fn(i32, u32) = |_, _| {};
    check(&f);
    let fr: fn(i32, u32) -> String = |_, _| String::new();
    check_ret(&fr);
    println!("  8. fn types + return type: OK");
}

// ------------------------------------------------------------
// 9. 属性支持：`#[allow(dead_code)]^T`
// ------------------------------------------------------------
#[batch_impl(#[allow(dead_code)]^usize, isize)]
trait AttrProbe {}

fn demos_attr() {
    fn check<T: AttrProbe>(_: &T) {}
    check(&0usize);
    check(&0isize);
    println!("  9. attribute injection: OK");
}

// ------------------------------------------------------------
// 10. `#` 指令：从 trait 自动读取签名
// ------------------------------------------------------------
#[batch_impl(
    usize #to_str{"usize"},
    isize #to_str{"isize"}
)]
trait ReadSig {
    fn to_str(&self) -> &'static str;
}

#[batch_impl(
    Vec<u32> #d_len{self.len()},
    Box^Vec^u32 #delegate(d_len){**self}
)]
trait DelegatedLen {
    fn d_len(&self) -> usize;
}

fn demos_directives() {
    assert_eq!(0usize.to_str(), "usize");
    assert_eq!(0isize.to_str(), "isize");
    let v: Vec<u32> = vec![1, 2, 3];
    assert_eq!(v.d_len(), 3);
    let b: Box<Vec<u32>> = Box::new(vec![1, 2, 3, 4]);
    assert_eq!(b.d_len(), 4);
    println!("  10. #name{{body}} + #delegate: OK");
}

// ------------------------------------------------------------
// 11. `batch_trait!` 多段、路径 trait、unsafe 段
// ------------------------------------------------------------
trait SegA {}
trait SegB<T> {}

/// # Safety
///
/// 演示用标记 trait，无实际 unsafe 语义。
unsafe trait SegUnsafe {}

mod deep {
    pub trait SegC {}
}

batch_trait!(
    SegA: u8, u16, u32;
    SegB: <T> SegB<T> Vec<T>;
    unsafe SegUnsafe: u32;
    deep::SegC: u32
);

fn demos_batch_trait_macro() {
    fn check_a<T: SegA>(_: &T) {}
    fn check_b<T: SegB<i32>>(_: &T) {}
    fn check_c<T: deep::SegC>(_: &T) {}
    fn check_u<T: SegUnsafe>(_: &T) {}
    check_a(&0u8);
    check_a(&0u16);
    check_a(&0u32);
    check_b(&vec![1i32, 2, 3]);
    check_c(&0u32);
    check_u(&0u32);
    println!("  11. batch_trait! multi-segment + path + unsafe: OK");
}

// ------------------------------------------------------------
// 12. `batch_impl_only`：trait 重复声明被丢弃
// ------------------------------------------------------------
trait DropTrait {
    fn val(&self) -> u32;
}

#[batch_impl_only(usize #val{42})]
trait DropTrait {
    fn val(&self) -> u32;
}

fn demos_batch_impl_only() {
    assert_eq!(0usize.val(), 42);
    println!("  12. batch_impl_only drops trait def: OK");
}

// ------------------------------------------------------------
// 13. 复杂类型透传：`dyn ...` 与 `fn() -> ...`
// ------------------------------------------------------------
#[batch_impl(
    &str,
    Box<dyn std::fmt::Display>,
    fn(i32) -> bool,
    (i32, String)
)]
trait ComplexMarker {}

fn demos_complex_passthrough() {
    fn check<T: ComplexMarker + ?Sized>(_: &T) {}
    check(&"hi");
    let b: Box<dyn std::fmt::Display> = Box::new(1i32);
    check(&b);
    let f: fn(i32) -> bool = |_| true;
    check(&f);
    check(&(1i32, String::from("x")));
    println!("  13. complex passthrough: OK");
}

// ------------------------------------------------------------
// 14. 嵌套泛型合并：`<T> Describe<T> [Vec<T>, <U> HashMap<T, U>]`
// ------------------------------------------------------------
#[batch_impl(<T> Describe<T> [Vec<T>, <U> HashMap<T, U>] {
    fn describe(&self) -> String { format!("len={}", self.len()) }
})]
trait Describe<T> {
    fn describe(&self) -> String;
}

fn demos_nested_generic_merge() {
    let v: Vec<i32> = vec![1, 2, 3];
    assert_eq!(v.describe(), "len=3");
    let m: HashMap<i32, String> = HashMap::from([(1, String::from("a"))]);
    assert_eq!(m.describe(), "len=1");
    println!("  14. nested generic merge (<A>[<B>T1,<C>T2]): OK");
}

// ============================================================
// 总览
// ============================================================

fn main() {
    println!("=== batch-impl quickstart ===");
    demos_basic();
    demos_generic();
    demos_shared_independent_body();
    demos_caret();
    demos_tuple_gen();
    demos_assoc_binding();
    demos_unsafe();
    demos_fn_types();
    demos_attr();
    demos_directives();
    demos_batch_trait_macro();
    demos_batch_impl_only();
    demos_complex_passthrough();
    demos_nested_generic_merge();
    println!("=== all demos ok ===");
}
