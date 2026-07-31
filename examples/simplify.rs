//! # 场景演示：一个"数据检查"小库，29 个 impl，约 15 行 DSL
//!
//! 手写这些 impl 需要 ~80 行：12 个数值类型各写一遍签名与 body、
//! 4 个包装类型各写一遍 `(**self).xxx()` 委托、元组各长度手写泛型、
//! fn/HashMap/指针/关联类型逐一实现……下面用 DSL 一键批量完成。
//!
//! 特性覆盖：`[...]` 并列列表、共享 body、`^` 列表应用、`&`/`*const`/`*mut`
//! 前缀、`where{...}` 约束、`#delegate` / `#fill` / `#name` 指令、
//! 元组生成 `()^1..=4`、`-` 左结合、关联类型绑定 `Item=T`、
//! `batch_impl` / `batch_impl_only` / `batch_trait!` 三个入口。

use batch_impl::{batch_impl, batch_impl_only, batch_trait};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

// ============================================================
// 1. 12 个数值类型：`[...]` 列表 + 共享 body → 12 impl
// ============================================================
// `Self::default()` 对整型/浮点都是 0，一个 body 通吃全部数值类型。
#[batch_impl(
    [u8, u16, u32, u64, usize, i8, i16, i32, i64, isize, f32, f64] {
        fn describe(&self) -> String { format!("num:{self}") }
        fn is_zero(&self) -> bool { *self == Self::default() }
    }
)]
trait Describe {
    fn describe(&self) -> String;
    fn is_zero(&self) -> bool;
}

// ============================================================
// 2. & / Box / Rc / Arc 委托内部值：`[&, Box, Rc, Arc]^T` → 4 impl
// ============================================================
// 为什么能合并成一条？四个类型的 `self` 都是"指向包装的引用"：
// - &T  ：self 是 &&T，`**self` = T
// - Box：self 是 &Box<T>，`**self` = T
// 所以委托体完全相同，`#delegate` 指令自动抄签名 + 生成 `(**self).method()`。
// `&` 与 Box/Rc/Arc 一样只是列表元素，`^` 把它们一次性应用到 T。
#[batch_impl_only(
    <T: Describe> [&, Box, Rc, Arc]^T #delegate(describe, is_zero){**self}
)]
trait Describe {
    fn describe(&self) -> String;
    fn is_zero(&self) -> bool;
}

// ============================================================
// 3. 元组生成：`()^1..=4` → 4 impl，各带独立泛型参数
// ============================================================
#[batch_impl(
    ()^1..=4 { fn describe(&self) -> &'static str { "tuple" } }
)]
trait DescribeTuple {
    fn describe(&self) -> &'static str;
}

// ============================================================
// 4. `-` 操作符（左结合，累加参数）：fn 返回类型 / HashMap<K, V>
// ============================================================
#[batch_impl(fn(i32, u32)-String)]
trait FnReturn {}

#[batch_impl(HashMap-u8-u16)]
trait KvMarker {}

// ============================================================
// 5. 关联类型绑定 `Item=T` + `#name{body}` 单 item（const）
// ============================================================
#[batch_impl(
    <T> IterInfo<Item=T> Vec<T> {
        fn describe(&self) -> String { format!("vec:{}", self.len()) }
    }
)]
trait IterInfo {
    type Item;
    fn describe(&self) -> String;
}

#[batch_impl(u8 #MAX{255})]
trait HasMax {
    const MAX: u8;
}

// ============================================================
// 6. `#fill(args){body}`：多方法共享同一 body
// ============================================================
#[batch_impl(u8 #fill(name, kind){"u8"})]
trait Kind {
    fn name(&self) -> &'static str;
    fn kind(&self) -> &'static str;
}

// ============================================================
// 7. `batch_trait!`：对已声明 trait 批量生成，多段 + unsafe 段
// ============================================================
trait Multi {}

/// # Safety
///
/// 仅演示 `unsafe` 段语法；本身无安全不变量需要说明。
unsafe trait UnsafeMark {}

batch_trait!(
    Multi: u8, u16;
    unsafe UnsafeMark: u32
);

// ============================================================
// 8. 指针前缀 `*const` / `*mut`
// ============================================================
#[batch_impl(*const^u32, *mut^i32)]
trait PtrMarker {}

// ============================================================
// 验证：29 个 impl（12 数值 + 4 包装 + 4 元组 + 2 fn/HashMap
// + 3 Multi/Unsafe + 2 关联类型/const + 2 指针）
// ============================================================
fn main() {
    // 1. 数值
    assert!(0u8.is_zero());
    assert_eq!(3i32.describe(), "num:3");
    assert!(0.0f64.is_zero());

    // 2. 包装（同一套委托体）
    assert!(Box::new(0u32).is_zero());
    assert!(!Rc::new(5u32).is_zero());
    assert!(!Arc::new(5u32).is_zero());
    assert_eq!(7u64.describe(), "num:7");
    assert!(!Box::new(3i32).is_zero());

    // 3. 元组
    assert_eq!((1u8,).describe(), "tuple");
    assert_eq!((1u8, 2u16, 3u32, 4u64).describe(), "tuple");

    // 4. fn 返回类型 / HashMap
    fn _f<T: FnReturn>(_: &T) {}
    fn _k<T: KvMarker>(_: &T) {}
    let fr: fn(i32, u32) -> String = |_, _| String::new();
    _f(&fr);
    _k(&HashMap::<u8, u16>::new());

    // 5. 关联类型 + const
    assert_eq!(vec![1u8, 2, 3].describe(), "vec:3");
    assert_eq!(<u8 as HasMax>::MAX, 255);

    // 6. #fill
    assert_eq!(0u8.name(), "u8");
    assert_eq!(0u8.kind(), "u8");

    // 7. batch_trait!
    fn _m<T: Multi>(_: &T) {}
    fn _u<T: UnsafeMark>(_: &T) {}
    _m(&0u8);
    _m(&0u16);
    _u(&0u32);

    // 8. 指针
    fn _p<T: PtrMarker>(_: &T) {}
    let c: *const u32 = &5u32;
    let m: *mut i32 = &mut 5i32;
    _p(&c);
    _p(&m);

    println!("✔ 约 15 行 DSL → 29 个 impl，全部断言通过");
}
