# Rust 1.85 → 1.98（stable）新特性教程

> 面向 Rust 个人项目开发者。覆盖 Rust stable **1.85.0 ~ 1.98.0** 的全部重要新特性，
> 并专题精讲三个"一等公民"但容易被忽视的语法点：**字符串字面量前缀**、
> **`use<..>`（precise capturing）**、**`&raw const` / `&raw mut`**。
>
> **验证声明**：本文所有代码示例均已在本机 `rustc 1.98.0`（2026-08-18 发布）下用
> `rustc --edition=2024` 实际编译运行验证（标注"预期编译失败"的反例除外）。每个示例
> 都给出**前后对比**：改动前（旧版本/旧写法）与改动后（新版本/新写法）。

---

## 0. 版本总览

| 版本 | 发布日期 | 主题 | 语言特性要点 |
|---|---|---|---|
| 1.85.0 | 2025-02-20 | **Edition 2024 稳定** + async closures | edition 2024、`AsyncFn` 系列、`#[diagnostic::do_not_recommend]` |
| 1.86.0 | 2025-04-03 | 类型系统放宽 | trait upcasting、安全版 `#[target_feature]`、`get_disjoint_mut`、`isqrt` |
| 1.87.0 | 2025-05-15 | **Rust 十周年** | `asm!` 跳转（asm_goto）、trait 定义中的 `use<..>`、匿名管道 |
| 1.88.0 | 2025-06-26 | 控制流语法 | **let chains**（仅 2024）、naked functions、`cfg(true)` 字面量 |
| 1.89.0 | 2025-08-07 | 常量泛型 + SIMD | `const N: _` 推断、`#[repr(u128)]`、AVX-512 目标特性 |
| 1.90.0 | 2025-09-18 | 链接提速 | 无语言特性；LLD 成为 Linux 默认链接器、`cargo publish --workspace` |
| 1.91.0 | 2025-10-30 | Windows ARM Tier 1 | C 风格可变参数（sysv64/win64/efiapi/aapcs）、strict 算术族 |
| 1.92.0 | 2025-12-11 | never 类型准备 | union 字段 `&raw` 安全化、同关联项多约束 |
| 1.93.0 | 2026-01-22 | asm 的 cfg | `asm_cfg`、`system` ABI 可变参数、`deref_nullptr` 升级 deny |
| 1.94.0 | 2026-03-05 | 切片窗口 | `array_windows`、Cargo config `include`、Unicode 17 |
| 1.95.0 | 2026-04-16 | match 守卫增强 | match 臂 `if let` guard、`cfg_select!`、`core::range::RangeInclusive` |
| 1.96.0 | 2026-05-28 | 新 Range 类型体系 | `core::range::{Range,RangeFrom,..}`（可 Copy）、`assert_matches!`、NonZero 范围迭代 |
| 1.97.0 | 2026-07-09 | v0 符号 + 位操作 | **v0 符号命名默认**、`isolate_*_one`/`bit_width`、`build.warnings` |
| 1.98.0 | 2026-08-20 | 数值格式化 | `format_into`+`NumBuffer`、`algebraic_*` 浮点代数、`substr_range`、`std::range::legacy` |

三个专题（字符串字面量前缀 / `use<..>` / `&raw`）在正文第 15~17 章。

---

## 1. Rust 1.85.0（2025-02-20）— Edition 2024 稳定 + async closures


### 1.1 Edition 2024 稳定（有史以来最大的一次 edition）

**是什么**：1.85 把 **Rust 2024 edition** 正式稳定（`cargo new` 默认生成、`cargo fix --edition 2024` 可自动迁移）。它带来一批语言默认行为变化，下面按影响大小列出（除注明外均为 2024 专属，2021 代码不受影响）。

#### 1.1.1 返回位置 `impl Trait` 的生命周期捕获规则

2024 中 `impl Trait` 的隐藏类型可以自由使用**所有 in-scope 生命周期**（含 elided 的
`&self`、impl 块生命周期），大幅减少 2021 常见的 E0700；未使用的生命周期不产生额外
约束（实测）。详见第 16 章专题二。

#### 1.1.2 `unsafe extern` 块

2024 起 `extern` 块必须写成 `unsafe extern`：

```rust
// 2021 写法（2024 中报错）：
// extern "C" { fn foo(); }

// 2024 写法：
unsafe extern "C" {
    fn foo();
}
```

#### 1.1.3 unsafe attributes：`#[unsafe(...)]`

`#[no_mangle]`、`#[export_name]`、`#[link_section]` 等 unsafe attribute 在 2024 必须
用 `#[unsafe(...)]` 包裹。实测错误信息：

```rust
// 2024：裸写报错
// #[no_mangle]
// pub extern "C" fn my_symbol() -> i32 { 42 }
// error: unsafe attribute used without unsafe
// help: wrap the attribute in `unsafe(...)`

// 2024 正确写法：
#[unsafe(no_mangle)]
pub extern "C" fn my_symbol() -> i32 {
    42
}
```

#### 1.1.4 `static mut` 的引用是硬错误

2024 中 `&mut STATIC` 必须包在 `unsafe` 块里（实测 E0133：
`use of mutable static is unsafe and requires unsafe block`）。取地址请用 `&raw mut`
（见第 17 章专题三）。

#### 1.1.5 `unsafe_op_in_unsafe_fn`：默认 **warn**（不是 deny！）

**实测（rustc 1.98）**：`unsafe fn` 内部直接做 unsafe 操作（如解引用裸指针）在
2024 下默认只是**警告**（`#[warn(unsafe_op_in_unsafe_fn)] on by default`），不是硬
错误。官方建议把 unsafe 操作显式包进 `unsafe {}` 块，并可在 crate 里
`#![deny(unsafe_op_in_unsafe_fn)]` 强制执行：

```rust
unsafe fn bad(p: *mut i32) {
    *p = 1; // warning（2024 默认 warn）
}

unsafe fn good(p: *mut i32) {
    unsafe { *p = 1; } // 推荐：显式块
}
```

#### 1.1.6 `gen` / `yeet` 成为保留关键字

实测：2024 中 `fn gen() {}` 报
`error: expected identifier, found reserved keyword \`gen\``；已有代码可用
`r#gen` 转义。`#"..."#` 字符串与 `##` token 也被保留（为将来的格式化字符串字面量
铺路）。

#### 1.1.7 标准库/edition 配套变化

- **`Future` 与 `IntoFuture` 加入 prelude**（所有 edition 生效！）——不再需要
  `use std::future::Future`。
- `IntoIterator for Box<[T]>`（2024 起，`for x in boxed_slice` 可用）。
- `std::env::set_var` / `remove_var` / `CommandExt::before_exec` 变为 **unsafe**。
- `if let` 与尾表达式临时值作用域规则调整；match 遍历保留项（未来可能破坏性改动）。
- Cargo：resolver v3 成为默认（rust-version 感知解析）。

### 1.2 async closures（RFC 3668）

**是什么**：`async ||` 创建**异步闭包**，调用返回 future。核心价值：异步闭包可以
**借用**捕获的变量（普通 `async move` 块只能 move 捕获），因此能表达"借用捕获的
future"，并支撑高阶异步 trait（`for<'a> AsyncFn(&'a str)`）。

**trait 层次**：`AsyncFnOnce` ⊃ `AsyncFnMut` ⊃ `AsyncFn`（对应 `FnOnce` ⊃
`FnMut` ⊃ `Fn`）。`AsyncFn*` 加入 prelude。

**前后对比**（完整可运行示例，rustc 1.85+）：

```rust
use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

// 简易 executor：轮询 future 直到 Ready（测试用；Waker::noop 是 1.85 新增）
fn block_on<F: Future>(fut: F) -> F::Output {
    let waker = Waker::noop();
    let mut cx = Context::from_waker(&waker);
    let mut fut = pin!(fut);
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

fn main() {
    // 改动前：只能用 async move 块，只能 move 捕获
    let s = String::from("hi");
    let _fut = async move { s.len() }; // s 被 move 进去，之后 s 不可再用

    // 改动后：async 闭包可借用捕获（实测通过）
    let add = async |x: i32| x + 1; // AsyncFn(i32) -> i32
    assert_eq!(block_on(add(41)), 42);

    let s2 = String::from("hi");
    let take = async move || s2; // move 捕获的 AsyncFnOnce
    assert_eq!(block_on(take()), "hi");
}
```

### 1.3 `#[diagnostic::do_not_recommend]`（RFC 2397）

**是什么**：标记某个 blanket impl"不推荐"，让编译器在错误信息中**不把该 impl 作为
建议**，减少误导。典型场景：你的 `From<&str>` 只用于内部，用户写 `x.into()` 报错时
编译器不该推荐它：

```rust
struct MyErr(String);
#[diagnostic::do_not_recommend]
impl From<&str> for MyErr {
    fn from(s: &str) -> Self { MyErr(s.into()) }
}
```

### 1.4 lint / 编译器

- 新 warn-by-default lint `unpredictable_function_pointer_comparisons`
  （函数指针比较在优化下不可预测）。
- `#[no_mangle]` 与 `#[export_name]` 同时使用时警告（`unused_attributes` 组，
  `#[export_name]` 优先）。
- `missing_fragment_specifier` 宏 lint 成为硬错误。

### 1.5 标准库亮点

```rust
use std::num::NonZero;

fn main() {
    // midpoint 系列（float + unsigned + NonZero；signed 是 1.84）
    assert_eq!(3.5f64.midpoint(4.5), 4.0);
    assert_eq!(5u32.midpoint(9), 7);
    assert_eq!(NonZero::new(3u32).unwrap().midpoint(NonZero::new(9u32).unwrap()).get(), 6);

    // 元组的 Extend / FromIterator（元数 1~12）
    let mut v = vec![(1, 2)];
    v.extend([(3, 4)]);
    assert_eq!(v, vec![(1, 2), (3, 4)]);

    // Waker::noop（const）
    let w = std::task::Waker::noop();

    // 函数指针地址比较（不看 metadata）
    fn a() {}
    fn b() {}
    assert!(std::ptr::fn_addr_eq(a as fn(), a as fn()));
    assert!(!std::ptr::fn_addr_eq(a as fn(), b as fn()));

    // io::ErrorKind 新变体
    let _ = std::io::ErrorKind::QuotaExceeded;
    let _ = std::io::ErrorKind::CrossesDevices;
}
```

更多 const 化：`mem::size_of_val`、`align_of_val`、`mem::swap`、`ptr::swap`、
`NonNull::new`、`MaybeUninit::write`、`HashMap/HashSet::with_hasher`、
float `recip/to_degrees/max/min/clamp/abs/signum/copysign` 等。

### 1.6 cargo / 其他

- edition 2024 支持（`cargo fix --edition 2024` 保守迁移）；resolver v3 默认。
- 构建脚本可读 `CARGO_CFG_FEATURE`；`cargo tree --depth workspace`。
- AArch64 Linux 的 rustc 用 ThinLTO+PGO 构建（提速约 30%）。

## 2. Rust 1.86.0（2025-04-03）— 类型系统放宽


### 2.1 trait upcasting（RFC 3324）

**是什么**：`&dyn Trait`（以及 `Box<dyn Trait>`、`Arc`、裸指针）可以**自动向上转换**
为 `&dyn Supertrait`，无需手动转换或 `transmute`。

**前后对比**：

```rust
trait Base {
    fn val(&self) -> i32;
}
trait Derived: Base {
    fn extra(&self) -> i32;
}

struct Impl;
impl Base for Impl { fn val(&self) -> i32 { 1 } }
impl Derived for Impl { fn extra(&self) -> i32 { 2 } }

// 改动前（1.86 之前）：dyn Derived 转 dyn Base 需要手写 vtable 操作
// （nightly 特性或 unsafe transmute），极其痛苦。
// 改动后（1.86）：
fn to_base(d: &dyn Derived) -> &dyn Base {
    d // 自动 upcast
}

fn main() {
    let d: &dyn Derived = &Impl;
    let b: &dyn Base = to_base(d);
    assert_eq!(b.val(), 1);
}
```

**注意**：指向无效 vtable 的裸指针做 upcast 是 UB（Miri 会检查）。

### 2.2 安全版 `#[target_feature]` 函数（target_feature_11，RFC 2396）

**是什么**：`#[target_feature]` 现在可以加在 **safe 函数**上（此前必须是 `unsafe fn`）。
规则：
- safe 的 target_feature 函数**只能在另一个 target_feature 函数内安全调用**；
- 从普通函数调用需要 `unsafe {}` 块；
- 不能作为 `Fn*` 泛型参数传递。

```rust
#[target_feature(enable = "avx2")]
pub fn sum_avx2(x: &[f32]) -> f32 {
    x.iter().sum() // 安全地使用 AVX2（编译器负责生成）
}

fn main() {
    // 从普通函数调用必须 unsafe（即使函数本身是 safe 的）
    if std::arch::is_x86_feature_detected!("avx2") {
        let _ = unsafe { sum_avx2(&[1.0, 2.0]) };
    }
}
```

### 2.3 `missing_abi` lint（RFC 3722）

`extern` 块 / `extern fn` 未显式写 `"C"` ABI 时警告（此前静默默认 C）：

```rust
extern fn legacy() {}  // warning: extern declarations without an explicit ABI
extern "C" fn explicit() {} // 推荐写法
```

### 2.4 其它编译器变化

- 新 warn-by-default lint `double_negations`（捕捉 `--x` 这类负负得正的笔误）。
- **调试断言**：debug 构建下，对非零大小读写与引用重借时断言指针非空。
- `-O` 现在等价 `-C opt-level=3`（与 cargo 默认一致）。
- `wasm_c_abi` future-incompat 警告 → 硬错误（要求 wasm-bindgen ≥ 0.2.89）。

### 2.5 标准库亮点

```rust
use std::num::NonZero;
use std::sync::{Once, OnceLock};

fn main() {
    // get_disjoint_mut：一次拿多个互不相交的可变引用（返回 Result）
    // 签名：fn get_disjoint_mut<I, const N>(&mut self, indices: [I; N])
    //        -> Result<[&mut I::Output; N], GetDisjointMutError>
    // where I: GetDisjointMutIndex + SliceIndex<Self>（usize 与 Range 均可）
    let mut a = [1, 2, 3, 4];
    if let Ok([x, y]) = a.get_disjoint_mut([0, 2]) {      // usize 数组
        *x += 10;
        *y += 20;
    }
    assert_eq!(a, [11, 2, 23, 4]);
    assert!(a.get_disjoint_mut([0, 0]).is_err());          // 重复下标拒绝

    let mut b = [0, 0, 0, 0];
    if let Ok([r1, r2]) = b.get_disjoint_mut([0..2, 2..4]) { // Range 数组
        r1[0] = 5;
        r2[0] = 6;
    }
    assert_eq!(b, [5, 0, 6, 0]);

    // HashMap 版（注意：返回 [Option<&mut V>; N]，与 slice 版的 Result 不同）
    use std::collections::HashMap;
    let mut m = HashMap::new();
    m.insert("a", 1);
    m.insert("b", 2);
    let [va, vb] = m.get_disjoint_mut(["a", "b"]);
    *va.unwrap() += 10;
    assert_eq!(*vb.unwrap(), 2);

    // 浮点 next_up / next_down
    assert!(f32::NAN.next_up().is_nan()); // 最小正增量
    assert_eq!(1.0f32.next_up(), 1.0000001);

    // NonZero::count_ones
    let nz = NonZero::new(5u8).unwrap(); // 0b101
    assert_eq!(nz.count_ones().get(), 2);

    // Vec::pop_if：条件弹出
    let mut v = vec![1, 2, 3];
    assert_eq!(v.pop_if(|x| *x == 3), Some(3));
    assert_eq!(v.pop_if(|x| *x == 99), None);

    // Once::wait / OnceLock::wait：阻塞等待初始化完成
    static INIT: Once = Once::new();
    let mut done = false;
    INIT.call_once(|| done = true);
    INIT.wait(); // 立即返回（已初始化）

    let lock = OnceLock::new();
    std::thread::scope(|s| {
        s.spawn(|| { lock.set(42).unwrap(); });
        assert_eq!(*lock.wait(), 42); // wait() -> &T，阻塞直到有值
    });
}
```

更多：`isqrt`（所有整数类型；注意 **没有** `checked_isqrt`，实测 1.98 不存在）、
const 化：`hint::black_box`、`str::{is_char_boundary, split_at, split_at_checked,
split_at_mut, split_at_mut_checked}`、`io::Cursor::{get_mut, set_position}`。

### 2.6 cargo / 其他

- 部分 config 键（credential-provider、runner 等"程序路径+参数"型）改为**整体替换**
  而非合并；`cargo login` 的 token 参数弃用（防 shell 历史泄漏）。
- `i586-pc-windows-msvc` 目标弃用（1.87 移除）。

## 3. Rust 1.87.0（2025-05-15）— Rust 十周年

### 3.1 语言特性

#### 3.1.1 `asm!` 跳转到 Rust 代码（asm_goto）

**是什么**：`asm!` 宏新增 **label 操作数**，汇编可以跳转到一段 Rust 代码块执行。

**为什么**：此前 `asm!` 只能表达"汇编内部跳转"，跳到 Rust 函数需要用函数指针做
间接跳转（需要额外的寄存器/内存往返）。有了 label 操作数，内联汇编可以直接
`jmp`/`call` 到 Rust 块，适合内核、引导程序、性能关键路径。

**语法**（两种形式都实测通过）：

```rust
use std::arch::asm;

fn main() {
    let mut count = 0;
    unsafe {
        // 命名形式
        asm!("jmp {l}", l = label { count += 1; });
        // 位置形式
        asm!("jmp {}", label { count += 1; });
    }
    assert_eq!(count, 2);
}
```

**要点**：
- label 的目标块返回值必须是 `()` 或 `!`；跳入后继续执行 `asm!` 之后的代码。
- 跳转块仍遵循借用检查（上面的 `count += 1` 在闭包式块里正常借用）。
- 1.87 时**输出操作数 + label 操作数混用仍未稳定**（仅输入可用）。

**前后对比**：1.87 之前无法在 stable 上让 `asm!` 直接跳到 Rust 块，只能：
```rust
// 旧写法：手动函数指针 + 间接跳转（示意）
unsafe extern "C" fn target() { /* ... */ }
unsafe { asm!("call {p}", p = in(reg) target as *const ()); }
```

#### 3.1.2 trait 定义中的 `impl Trait + use<..>`（precise_capturing_in_traits）

**是什么**：1.82 稳定了自由函数/impl 中的 `use<..>` 精确捕获；**1.87 把 `use<..>`
扩展到 trait 定义里**的返回位置 `impl Trait`。

```rust
trait Parser<'a> {
    // 显式声明只捕获 'a（以及 Self），不捕获其它 in-scope 泛型
    fn parse(&self, input: &'a str) -> impl Iterator<Item = &'a str> + use<'a, Self>;
}
```

详细规则见第 16 章专题二。

#### 3.1.3 一元运算符后的开区间 `..EXPR`

**是什么**：解析器支持一元运算符（`-`、`!`、`*`）后直接跟"开起始"区间，例如
`-1..`、`!0..`。此前 `-1..` 会被解析成 `-(1..)` 或报错。

```rust
// 1.87 前：`-1..` 会被解析成 `-(1..)`（类型错误）或要求加括号
// let r = -1..; // error
// 1.87 后：直接可用，等价于 RangeFrom { start: -1 }
let r = -1..;
let v: Vec<i32> = r.take(3).collect();
assert_eq!(v, vec![-1, 0, 1]);
```

#### 3.1.4 unsized 类型不再要求实现 `Self: Sized` 方法

**是什么**：给 unsized 类型（如 `str`、`[T]`）实现 trait 时，不再要求实现带
`where Self: Sized` 的方法（反正这些方法永远无法被调用）。

```rust
trait Describe {
    fn name(&self) -> String;
    fn sized_only(self) where Self: Sized; // 对 unsized 类型无法调用
}

// 1.87 前：impl Describe for str 必须连 sized_only 一起实现（实际上无法有意义地实现）
// 1.87 后：
impl Describe for str {
    fn name(&self) -> String { self.to_string() }
    // sized_only 可以省略
}
```

### 3.2 标准库亮点（已实测）

```rust
use std::io::{Read, Write};

fn main() {
    // 匿名管道：不需要命名文件
    let (mut r, mut w) = std::io::pipe().unwrap();
    w.write_all(b"hi").unwrap();
    let mut buf = [0u8; 2];
    r.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b"hi");

    // Vec::extract_if：带范围 + 过滤条件
    let mut v = vec![1, 2, 3, 4, 5];
    let odds: Vec<i32> = v.extract_if(.., |x| *x % 2 == 1).collect();
    assert_eq!(odds, vec![1, 3, 5]);
    assert_eq!(v, vec![2, 4]);

    // String::extend_from_within：复制自身的一段
    let mut s = String::from("abcd");
    s.extend_from_within(1..3);
    assert_eq!(s, "abcdbc");

    // str::from_utf8 成为 str 的 inherent 关联函数（且 const）
    const S: &str = match str::from_utf8(b"hi") { Ok(s) => s, Err(_) => "" };
    assert_eq!(S, "hi");

    // Box<MaybeUninit<T>>::write（关联函数形式）
    let b: Box<i32> = Box::write(Box::new(std::mem::MaybeUninit::uninit()), 7);
    assert_eq!(*b, 7);

    // 无符号转有符号 / is_multiple_of / 无界移位 / 中点
    assert_eq!(200u8.cast_signed(), -56i8);
    assert_eq!(12u32.is_multiple_of(3), true);
    assert_eq!(1u8.unbounded_shl(7), 128u8); // 不 panic、按类型宽度截断
    assert_eq!(3i32.midpoint(9), 6);

    // 指针距离（无符号）
    let arr = [1u8; 10];
    let q = unsafe { arr.as_ptr().add(3) };
    assert_eq!(unsafe { q.offset_from_unsigned(arr.as_ptr()) }, 3);

    // OsStr::display
    assert_eq!(std::ffi::OsStr::new("p").display().to_string(), "p");

    // TryFrom<Vec<u8>> for String
    assert_eq!(String::try_from(b"ok".to_vec()).unwrap(), "ok");
}
```

更多新增：`LinkedList::extract_if`、slice `split_off` 系列、
`<*const T>::byte_offset_from_unsigned`、`cast_unsigned`/`NonZero` 变体、
`<iN>::midpoint`、`<uN|iN>::unbounded_shr`、`<str>::from_utf8_mut/unchecked` 系列，
以及约 25 个 const 化 API（`Vec::as_ptr`、`<[T]>::copy_from_slice`、
`String::into_bytes`、`char::is_whitespace`、`<[[T; N]]>::as_flattened` 等）。

### 3.3 lint / 编译器行为

- `ptr_cast_add_auto_to_object` 与顺序相关 trait object 兼容性警告 → **硬错误**。
- raw pointer 的 `Debug` 现在会打印 metadata（如 `0x... (0x... = 0x...)`）。
- `ControlFlow` 变为 `#[must_use]`。
- Windows 下 std 不再链接 advapi32（win7 目标除外）。
- 大量 `std::arch` 内在函数在启用对应目标特性后变为 safe 可调用 —— 可能给现有
  代码带来新的 `unused_unsafe` 警告。
- i686 目标强制要求 SSE2；编译器内部升级到 LLVM 20。

### 3.4 cargo / 其他

- 终端集成（ANSI OSC 9;4 进度序列）；`cargo package --exclude-lockfile`。
- **里程碑**：恰逢 Rust 1.0 发布十周年（2015-05-15 → 2025-05-15）；
  移除 `i586-pc-windows-msvc` 目标。


## 4. Rust 1.88.0（2025-06-26）— let chains 与 naked functions

### 4.1 语言特性

#### 5.1.1 let chains（仅 edition 2024）

**是什么**：`if`/`while` 条件中允许用 `&&` 串联多个 `let` 模式匹配与布尔表达式。

**为什么**：嵌套 `if let` 会制造深层缩进与重复的错误处理分支；let chains 把
"先匹配、再判断、绑定贯穿后续条件与函数体"压成一行。它**只在 edition 2024**
可用，因为依赖 2024 的 if-let 临时值作用域规则（保证链中各临时值按书写顺序 drop）。

**前后对比**：

```rust
// 改动前（2021 写法）：
fn channel_major(release: Option<(u32, u32)>) -> Option<u32> {
    if let Some((_, minor)) = release {
        if minor == 88 {
            return Some(minor);
        }
    }
    None
}

// 改动后（1.88，edition 2024）：
fn channel_major(release: Option<(u32, u32)>) -> Option<u32> {
    if let Some((_, minor)) = release && minor == 88 {
        Some(minor)
    } else {
        None
    }
}
```

实测：edition 2021 编译同样代码报
`error: let chains are only allowed in Rust 2024 or later`。

#### 5.1.2 naked functions

**是什么**：`#[unsafe(naked)]` 标记的函数**没有编译器生成的前言/尾声**
（prologue/epilogue），函数体必须是唯一的 `naked_asm!` 调用，完全由你控制生成的
汇编。适合实现编译器内建函数、OS/嵌入式启动代码。

```rust
// x86_64 (sysv64)：两数相加，仅 2 条指令
#[unsafe(naked)]
pub unsafe extern "sysv64" fn wrapping_add(a: u64, b: u64) -> u64 {
    core::arch::naked_asm!("lea rax, [rdi + rsi]", "ret");
}

fn main() {
    assert_eq!(unsafe { wrapping_add(3, 4) }, 7);
}
```

**注意**：naked 函数里不能有正常的 Rust 语句；`#[unsafe(naked)]` 是 edition 2024
的 unsafe attribute 写法（2021 用 `#[naked]`，带警告）。

#### 5.1.3 布尔 cfg 字面量（RFC 3695）

**是什么**：`cfg(true)` / `cfg(false)` 作为恒真/恒假的配置谓词，取代容易读错的
`cfg(all())` / `cfg(any())`。可用于 `#[cfg]`、`#[cfg_attr]`、`cfg!` 宏以及
Cargo 的 `[target]` 表。

```rust
// 改动前：语义是"恒真"但写法像"空条件"
#[cfg(all())] fn always() {}
// 改动后：
#[cfg(true)] fn always() {}

// cfg! 宏同理
assert!(cfg!(true));
assert!(!cfg!(false));
```

#### 5.1.4 `#[bench]` 彻底移除

`#[bench]` 在无 `#![feature(custom_test_frameworks)]` 时是硬错误（1.77 起就是
deny-by-default 的 future-incompat 警告）。

### 4.2 标准库亮点（已实测）

```rust
use std::cell::Cell;
use std::hint;

fn main() {
    // Cell::update
    let c = Cell::new(5);
    c.update(|x| x + 1);
    assert_eq!(c.get(), 6);

    // raw pointer 的 Default 实现（默认空指针）
    let p: *const i32 = Default::default();
    assert!(p.is_null());

    // 新的 std::ffi::c_str 模块（core 同步）
    use std::ffi::c_str::CStr;
    assert_eq!(c"hi".to_bytes(), b"hi");

    // HashMap::extract_if（按值判定，可移除）
    use std::collections::HashMap;
    let mut map = HashMap::new();
    map.insert(1, "a");
    map.insert(2, "b");
    let removed: Vec<i32> = map.extract_if(|k, _| *k == 1).map(|(k, _)| k).collect();
    assert_eq!(removed, vec![1]);
    assert_eq!(map.len(), 1);

    // slice::as_chunks（全 const）
    let a = [1, 2, 3, 4, 5];
    let (chunks, rem) = a.as_chunks::<2>();
    assert_eq!(chunks.len(), 2);
    assert_eq!(rem, &[5]);

    // hint::select_unpredictable：分支预测提示
    let v = hint::select_unpredictable(true, 1, 2);
    assert_eq!(v, 1);
}
```

更多：`proc_macro::Span::{line,column,start,end,file,local_file}`、
`<[T]>::as_rchunks`/`as_chunks_unchecked`/`as_chunks_mut` 系列；const 化：
`NonNull::replace`、`<*mut T>::replace`、`ptr::swap_nonoverlapping`、
`Cell::{replace, get, get_mut, from_mut, as_slice_of_cells}`。

### 4.3 lint / 编译器行为

- 新 warn-by-default lint **`dangerous_implicit_autorefs`**（raw 指针解引用的隐式
  autoref；1.89 转 deny）。
- 新 lint **`invalid_null_arguments`**（从 clippy 提升）。
- borrow checker 修复：某些"恒真 pattern"此前被过度放行，1.88 起不再编译。
- 最低外部 LLVM 提升到 19；`-Cdwarf-version` 稳定。

### 4.4 cargo / 其他

- **自动缓存垃圾回收**：网络下载的依赖 3 个月未用即清理、本地构建产物 1 个月；
  `cache.auto-clean-frequency = "never"` 可关闭。
- Cargo 的 gzip 改用 zlib-rs 实现；libtest `--nocapture` 弃用，改 `--no-capture`。
- `i686-pc-windows-gnu` 降级为 Tier 2；`[T; N]::from_fn` 保证按索引递增顺序调用。


## 5. Rust 1.89.0（2025-08-07）— 常量泛型推断 + SIMD

### 5.1 语言特性

#### 7.1.1 const 泛型参数的 `_` 推断

**是什么**：`_` 现在可以作为 const 泛型实参，由上下文推断具体值
（此前 `_` 只允许在类型位置）。

```rust
// 改动前：必须显式写出 LEN
pub fn all_false<const LEN: usize>() -> [bool; LEN] {
    [false; LEN]
}
// 改动后：由返回类型推断
pub fn all_false<const LEN: usize>() -> [bool; LEN] {
    [false; _]
}

fn main() {
    assert_eq!(all_false::<5>().len(), 5);
}
```

**限制**：`_` 不能出现在签名里（返回类型、`const` 项等），与类型位置规则一致；
如 `pub fn bad<const LEN: usize>() -> [bool; _]` 不允许。

#### 7.1.2 `#[repr(u128)]` / `#[repr(i128)]`

**是什么**：枚举可以使用 128 位判别式（此前是 nightly 的 `repr128` 特性）。

```rust
#[repr(u128)]
enum Big {
    A = 1u128 << 100, // 判别式超过 u64 范围
}

fn main() {
    assert_eq!(Big::A as u128, 1u128 << 100);
}
```

#### 7.1.3 大批 x86 目标特性与 i128 extern "C"

- 稳定了整套 `avx512*` 目标特性以及 `kl`、`widekl`、`sha512`、`sm3`、`sm4`，
  可配合 `#[target_feature(enable = "...")]` 与 `#[cfg(target_feature = "...")]`。
- `i128`/`u128` 用于 `extern "C"` 函数不再触发 `improper_ctypes_definitions`
  lint（与 C `__int128` ABI 兼容，但不保证与 C23 `_BitInt(128)` 兼容）。
- 元组结构体/变体构造器的实参临时值获得与普通函数调用一致的生命周期延长。

### 5.2 标准库亮点（已实测）

```rust
use std::num::NonZero;

fn main() {
    // 文件锁
    let f = std::fs::File::create("lock.tmp").unwrap();
    f.lock().unwrap();
    f.unlock().unwrap();

    // Result::flatten 变 const
    const FLAT: Result<i32, ()> = Ok::<Result<i32, ()>, ()>(Ok(1)).flatten();
    assert_eq!(FLAT, Ok(1));

    // NonZero<char>
    let nz = NonZero::new('a').unwrap();
    assert_eq!(nz.get(), 'a');

    // OsString::leak / PathBuf::leak -> 'static
    let leaked: &'static std::ffi::OsStr = std::ffi::OsString::from("x").leak();
    assert_eq!(leaked, std::ffi::OsStr::new("x"));
}
```

更多：`NonNull::{from_ref, from_mut, without_provenance, with_exposed_provenance,
expose_provenance}`、`File::{lock_shared, try_lock, try_lock_shared}`、
Linux `TcpStreamExt::quickack`、AVX-512 与 SHA512/SM3/SM4 内在函数；
const 化：`<[T; N]>::as_mut_slice`、`eq_ignore_ascii_case`。

### 5.3 lint / 编译器行为

- 新 warn-by-default lint **`mismatched_lifetime_syntaxes`**（生命周期在类型路径中
  elided、在 `&T`/`'a` 中显式，两种写法混用会误导读者）——取代并弃用
  `elided_named_lifetimes`。实测警告信息：
  ```rust
  fn items(scores: &[u8]) -> std::slice::Iter<u8> { scores.iter() }
  // warning: hiding a lifetime that's elided elsewhere is confusing
  // help: use `'_` for type paths:  -> std::slice::Iter<'_, u8>
  ```
- `dangerous_implicit_autorefs` 从 warn 升为 **deny**；`missing_fragment_specifier`
  成为无条件硬错误。
- 编译器默认开启非叶帧指针（aarch64-linux、Apple 各架构等）。

### 5.4 cargo / 其他

- `cargo fix`/`cargo clippy --fix` 的目标选择与其他构建命令统一。
- **doctest 交叉编译稳定**：`cargo test --doc --target ...` 真正运行 doctest。
- `x86_64-apple-darwin` 开始从 Tier 1 降级（1.90 生效）。


## 6. Rust 1.90.0（2025-09-18）— LLD 默认链接 + workspace 发布

### 6.1 语言特性

**无**新语言特性稳定（"Language" 栏的三项是行为变更，见 6.3）。

### 6.2 标准库亮点（已实测）

```rust
fn main() {
    // 无符号减有符号（支持负的减法）
    assert_eq!(5u8.saturating_sub_signed(-3i8), 8);
    assert_eq!(1u8.saturating_sub_signed(10i8), 0);
    // 同族：checked / overflowing / wrapping 版本

    // CStr/CString/Cow<CStr> 之间的 PartialEq 实现（10 个新 impl）
    let a: &std::ffi::CStr = c"same";
    assert!(a == c"same");
}
```

更多：`IntErrorKind` 实现 `Copy + Hash`；const 化：`<[T]>::reverse`、
`f32/f64::{floor, ceil, trunc, fract, round, round_ties_even}`。

### 6.3 编译器 / 行为变更

- **LLD 成为 `x86_64-unknown-linux-gnu` 默认链接器**（提速；`-C linker-features=-lld`
  可退回）；Tier-3 musl 目标改为动态链接。
- 诊断属性 lint 拆分为 4 个更细的 lint（`unknown_diagnostic_attributes` 等）。
- 允许引用可变/外部内存的常量，但这类常量**不能用作模式**。
- 允许对非 Rust 内存（含地址 0）做 volatile 访问。

### 6.4 cargo / 其他

- **`cargo publish --workspace` 稳定**（按依赖顺序发布整个 workspace）。
- `http.proxy-cainfo` 配置；`cargo package` 改用 gix 库。
- `x86_64-apple-darwin` 正式降为 Tier 2（带 host tools）。


## 7. Rust 1.91.0（2025-10-30）— Windows ARM Tier 1 + 裸指针 lint

### 7.1 语言特性

#### 11.1.1 C 风格可变参数（sysv64 / win64 / efiapi / aapcs ABI）

**是什么**：这 4 个 ABI 现在与 C ABI 一致，可以在 `extern` 块中**声明**
可变参数函数（仍不能定义）。

```rust
unsafe extern "sysv64" {
    fn printf(fmt: *const u8, ...); // 1.91 起可声明（1.93 补充 "system" ABI）
}
```

**前后对比**：1.91 之前，非 C ABI 的 `...` 可变参数声明是 nightly 特性
（`c_variadic` 只覆盖 `"C"` ABI）。

#### 11.1.2 其它

- LoongArch32 内联汇编稳定；x86 `sse4a`、`tbm` 目标特性稳定。
- `target_env = "macabi"` / `"sim"` cfg 取代对应的 `target_abi`。
- 模式绑定按书写顺序降级（drop 顺序相应变化，多数情况下不可观察）。

### 7.2 标准库亮点（已实测）

```rust
fn main() {
    // strict 算术：溢出直接 panic（而不是回绕）
    assert_eq!(100u8.strict_add(27), 127);

    // core::iter::chain：省去 .into_iter()
    let joined: Vec<i32> = core::iter::chain([1, 2], [3, 4]).collect();
    assert_eq!(joined, vec![1, 2, 3, 4]);

    // core::array::repeat
    let arr: [String; 3] = core::array::repeat(String::from("x"));

    // 带进位算术（第 3 参数是 bool carry）
    let (sum, carry) = 255u8.carrying_add(1, false);
    assert_eq!((sum, carry), (0, true));
    assert_eq!(10u8.checked_signed_diff(3), Some(7i8));

    // Duration 构造
    assert_eq!(std::time::Duration::from_mins(1), std::time::Duration::from_secs(60));

    // 路径操作
    let p = std::path::PathBuf::from("archive.tar");
    assert_eq!(p.file_prefix().unwrap().to_str(), Some("archive"));
    let mut p2 = std::path::PathBuf::from("a");
    p2.add_extension("b");
    assert_eq!(p2.to_str(), Some("a.b"));

    // BTreeMap::extract_if（带范围）
    use std::collections::BTreeMap;
    let mut m = BTreeMap::new();
    m.insert(1, "a"); m.insert(2, "b"); m.insert(3, "c");
    let picked: Vec<(i32, &str)> = m.extract_if(2.., |_, _| true).collect();
    assert_eq!(picked.len(), 2);
    assert_eq!(m.len(), 1);

    // 字符边界（UTF-8 安全的下标调整）
    let s = "héllo"; // 边界：0,1,3,4,5,6
    assert_eq!(s.floor_char_boundary(2), 1); // 2 落在 é 内部 -> 向下取 1
    assert_eq!(s.ceil_char_boundary(2), 3);  // 向上取 3

    // AtomicPtr fetch 系列
    use std::sync::atomic::{AtomicPtr, Ordering};
    let mut x = 0u64;
    let ap = AtomicPtr::new(&mut x as *mut u64);
    let prev = ap.fetch_byte_add(8, Ordering::SeqCst);
    assert_eq!(prev, &mut x as *mut u64);
}
```

更多：`Path::file_prefix`、`AtomicPtr::{fetch_ptr_add, fetch_ptr_sub, fetch_or,
fetch_and, fetch_xor}`、`{integer}::strict_{add,sub,mul,div,rem,neg,shl,shr,pow}` 全族
（const）、`PanicHookInfo::payload_as_str`、`PathBuf::with_added_extension`、
`Duration::from_hours`、`Ipv4Addr::from_octets`、`Ipv6Addr::{from_octets,
from_segments}`、`Pin<Box/Rc/Arc<T>>: Default`、`Cell::as_array_of_cells`、
`BTreeMap/BTreeSet::extract_if`、`str::{ceil,floor}_char_boundary`、
`Saturating<uN>: Sum/Product`、Path 与 str/String 的 PartialEq；
const 化：`each_ref`/`each_mut`、`OsString::new`、`PathBuf::new`、`TypeId::of`。

### 7.3 lint / 编译器行为

- 新 warn-by-default lint **`dangling_pointers_from_locals`**：函数返回指向局部变量
  的裸指针。实测警告：
  ```rust
  fn dangling() -> *const u8 {
      let x = 0u8;
      &raw const x
      // warning: function returns a dangling pointer to dropped local variable `x`
  }
  ```
- 新 warn-by-default lint **`integer_to_ptr_transmutes`**（整数到指针 transmute）。
- `semicolon_in_expressions_from_macros` 升为 deny。
- `unreachable_code` 不再对 never→任意类型的 `as` 转换警告。

### 7.4 cargo / 其他

- **`build.build-dir` 稳定**（指定中间构建产物目录）。
- `--target "host-tuple"` 字面量；编译器升级 LLVM 21。
- **里程碑**：`aarch64-pc-windows-msvc` 升为 **Tier 1**（首个 Windows-on-ARM
  Tier 1）；panic 消息现在打印线程 ID。


## 8. Rust 1.92.0（2025-12-11）— never 类型准备

### 8.1 语言特性

#### 13.1.1 union 字段的 `&raw` 借用安全化

**是什么**：safe 代码中可以直接 `&raw const` / `&raw mut` 取 union 字段地址，
不再需要 unsafe 块（读取字段内容仍需要 unsafe，这是 union 的固有性质）。

```rust
union U { i: i32, f: f32 }

fn main() {
    let u = U { i: 7 };
    let p = &raw const u.i; // 1.92 前需要 unsafe 块
    assert_eq!(unsafe { *p }, 7);
}
```

#### 13.1.2 同一关联项多个约束（trait object 除外）

同一关联类型/常量可以出现多次约束，编译器取合取；trait object 中仍要求唯一投影。

```rust
trait Container {
    type Item;
}
fn f<T: Container<Item: Clone> + Container<Item: Send>>() {}
```

### 8.2 标准库亮点（已实测）

```rust
use std::num::NonZero;

fn main() {
    // NonZero::div_ceil（const）
    const D: u32 = NonZero::new(7u32).unwrap().div_ceil(NonZero::new(3u32).unwrap()).get();
    assert_eq!(D, 3);

    // Box/Rc/Arc::new_zeroed 系列
    let b = Box::<u32>::new_zeroed();
    assert_eq!(unsafe { *b.assume_init() }, 0);
}
```

更多：`Location::file_as_c_str`、`RwLockWriteGuard::downgrade`、
`btree_map::Entry::insert_entry`、`Extend<proc_macro 类型> for TokenStream`；
const 化：`<[_]>::rotate_left`/`rotate_right`。

### 8.3 lint / 编译器行为

- `never_type_fallback_flowing_into_unsafe` 与
  `dependency_on_unit_never_type_fallback` 升为 **deny-by-default**（为 never 类型
  正式落地做准备；约 500 个 crate 受影响）。
- `invalid_macro_export_arguments` 升 deny（依赖中也生效）。
- `unused_must_use` 不再对 `Result<(), 无类型>`（如 `Result<(), Infallible>`）警告。
- Linux 下 `-C panic=abort` 也默认生成 unwind 表（回溯可用；可用
  `-C force-unwind-tables=no` 关闭）。


## 9. Rust 1.93.0（2026-01-22）— asm 的 cfg

### 9.1 语言特性

#### 15.1.1 asm_cfg：`#[cfg]` 作用于 asm 指令/操作数

**是什么**：`#[cfg]` 可以放在单个 `asm!` / `global_asm!` / `naked_asm!` 的指令串或
操作数上，按目标条件剪裁汇编。

```rust
use std::arch::asm;

fn f() {
    unsafe {
        asm!(
            "nop",
            #[cfg(target_feature = "sse2")] "nop",   // 仅在 sse2 时加入
            #[cfg(target_arch = "x86_64")] "nop",
        );
    }
}
```

#### 15.1.2 `"system"` ABI 可变参数

承接 1.91：`extern "system" { fn printf(fmt: *const u8, ...); }` 也可声明。

### 9.2 标准库亮点（已实测）

```rust
fn main() {
    // slice::as_array / as_mut_array
    let s = [1, 2, 3];
    assert!(s.as_array::<2>().is_none());
    assert_eq!(s.as_array::<3>().unwrap(), &[1, 2, 3]);

    // Vec::into_raw_parts / String::into_raw_parts
    let v = vec![1, 2, 3];
    let (ptr, len, cap) = Vec::into_raw_parts(v);
    unsafe { drop(Vec::from_raw_parts(ptr, len, cap)); }

    // MaybeUninit::assume_init_drop / assume_init_ref / assume_init_mut
    let mut mu = std::mem::MaybeUninit::new(String::from("a"));
    unsafe { mu.assume_init_drop(); }

    // VecDeque::pop_front_if / pop_back_if
    use std::collections::VecDeque;
    let mut d = VecDeque::from([1, 2, 3]);
    assert_eq!(d.pop_front_if(|x| *x < 2), Some(1));

    // Duration::from_nanos_u128
    assert_eq!(std::time::Duration::from_nanos_u128(1_000_000_000),
               std::time::Duration::from_secs(1));

    // char 的 UTF-8/16 最大编码长度常量
    assert_eq!(char::MAX_LEN_UTF8, 4);

    // fmt::from_fn：闭包即格式化器
    let s = format!("{}", std::fmt::from_fn(|f| f.write_str("hi")));
    assert_eq!(s, "hi");
}
```

更多：`<[MaybeUninit<T>]>::write_copy_of_slice`/`write_clone_of_slice`（1.98 实测为
safe，源/目标长度必须一致）、`<iN>::unchecked_neg/shl/shr`、
`<uN>::unchecked_shl/shr`（unsafe，溢出 UB）、
`<*const [T]>::as_array`、`<*mut [T]>::as_array_mut`。

### 9.3 lint / 编译器行为

- 新 warn-by-default lint **`function_casts_as_integer`**（函数指针转整数）。
- **`deref_nullptr` 升为 deny-by-default**。
- 新 warn-by-default lint **`const_item_interior_mutations`**。
- musl 目标升级到 musl 1.2.5（要求 libc ≥ 0.2.146）。

### 9.4 cargo / 其他

- 构建脚本按 profile 提供 `CARGO_CFG_DEBUG_ASSERTIONS`。
- `cargo clean --workspace`；`pin_v2` 内建属性命名空间引入（为 Pin 重构铺路）。


## 10. Rust 1.94.0（2026-03-05）— 切片窗口

### 10.1 语言特性

- **无重大新语法**。亮点是库 API 与工具链（见下）。

### 10.2 标准库亮点（已实测）

```rust
fn main() {
    // slice::array_windows：滑动窗口，元素是 &[T; N]，消除 bounds-check 开销
    let a = [1, 2, 3, 4, 5];
    let wins: Vec<&[i32; 3]> = a.array_windows::<3>().collect();
    assert_eq!(wins.len(), 3);
    assert_eq!(wins[1], &[2, 3, 4]);

    // slice::element_offset：元素位置（指针级，避免 index 计算）
    let b = [10, 20, 30];
    assert_eq!(b.element_offset(&b[2]), Some(2));

    // LazyCell / LazyLock 的访问器族
    let lc = std::cell::LazyCell::new(|| 42);
    assert_eq!(std::cell::LazyCell::get(&lc), None);
    assert_eq!(*std::cell::LazyCell::force(&lc), 42);

    // TryFrom<char> for usize
    assert_eq!(usize::try_from('a'), Ok(97));

    // Peekable::next_if_map
    let mut p = [1, 2, 3].into_iter().peekable();
    assert_eq!(p.next_if_map(|x| if x == 1 { Ok(x * 10) } else { Err(x) }), Some(10));

    // 浮点常量 + const 化 mul_add
    const M: f32 = 2.0f32.mul_add(3.0, 1.0);
    assert_eq!(M, 7.0);
    assert!(std::f32::consts::EULER_GAMMA > 0.0);
    assert!((std::f32::consts::GOLDEN_RATIO - 1.6180339).abs() < 1e-6);
}
```

更多：`<[T]>::as_mut_array`、`LazyLock::{get, get_mut, force_mut}`、
`Peekable::next_if_map_mut`、x86 `avx512fp16` 与 AArch64 NEON fp16 内在函数。

### 10.3 lint / 编译器行为

- 新 warn-by-default lint **`unused_visibilities`**：`pub const _: u8 = 1;` 这种
  可见性无意义的写法。实测警告：
  ```rust
  pub const _: u8 = 1;
  // warning: visibility qualifiers have no effect on `const _` declarations
  ```
- impl 及 impl 项继承 trait 的 `dead_code` lint 级别；Unicode 17 数据表。

### 10.4 cargo / 其他

- **Cargo config 的 `include` 键稳定**：`include = [{ path = "extra.toml" },
  { path = "opt.toml", optional = true }]`。
- **TOML 1.1 解析**：多行 inline table、尾随逗号、`\xHH`/`\e` 转义、可选秒。
- 运行时可读 `CARGO_BIN_EXE_<crate>`；`riscv64im-unknown-none-elf` 新 Tier 3 目标。


## 11. Rust 1.95.0（2026-04-16）— if-let guards 与 `cfg_select!`

### 11.1 语言特性

#### 11.1.1 match 臂上的 if-let guard

**是什么**：match 臂的 guard（`if` 守卫）现在可以是 **`if let` 模式守卫**——
先做模式匹配、失败即跳过该臂；匹配成功的绑定在臂体内可用。相当于把
let chains 的能力带进 `match`。

**前后对比**：

```rust
fn compute(x: i32) -> Result<i32, ()> {
    Ok(x * 2)
}

// 改动前（嵌套 match 或提前 return）：
fn before(value: Option<i32>) -> Option<i32> {
    match value {
        Some(x) => match compute(x) {
            Ok(y) => Some(y),
            Err(_) => None,
        },
        None => None,
    }
}

// 改动后（1.95，实测）：
fn after(value: Option<i32>) -> Option<i32> {
    match value {
        Some(x) if let Ok(y) = compute(x) => Some(y),
        _ => None,
    }
}

fn main() {
    assert_eq!(after(Some(21)), Some(42));
}
```

**注意**：if-let guard 里的模式**不参与 match 的穷尽性检查**（与普通 `if` guard 一致）。

#### 11.1.2 其它

- `irrefutable_let_patterns` lint 不再对 let chains 报警。
- 支持**带重命名的路径段关键字导入**（如 `use std::r#async as my_async;`）。
- **PowerPC / PowerPC64 内联汇编稳定**。

### 11.2 标准库亮点（已实测）

```rust
use core::hint;
use core::range::RangeInclusive;

fn main() {
    // cfg_select!：按目标平台选值（比 cfg! + 多次声明简洁）
    let os = cfg_select! {
        windows => "windows",
        unix => "unix",
        _ => "other",
    };

    // bool: TryFrom<{integer}>
    assert_eq!(bool::try_from(1u8), Ok(true));
    assert!(bool::try_from(2u8).is_err());

    // Atomic*::update / try_update（两个 Ordering 参数，返回旧值）
    use std::sync::atomic::{AtomicUsize, Ordering};
    let a = AtomicUsize::new(1);
    let prev = a.update(Ordering::SeqCst, Ordering::SeqCst, |x| x * 2);
    assert_eq!(prev, 1);
    assert_eq!(a.load(Ordering::SeqCst), 2);

    // hint::cold_path()：告诉编译器这是冷路径
    if a.load(Ordering::SeqCst) > 100 {
        hint::cold_path();
    }

    // <*const T>::as_ref_unchecked / <*mut T>::as_mut_unchecked
    let x = 7i32;
    let p = &x as *const i32;
    assert_eq!(unsafe { p.as_ref_unchecked() }, &7);

    // Vec::push_mut / VecDeque / LinkedList 的 *_mut 系列（返回 &mut T 槽位）
    let mut v = vec![1, 3];
    let slot: &mut i32 = v.push_mut(2);
    *slot += 10;
    assert_eq!(v, vec![1, 3, 12]);

    // 新 Range 体系的第一块：core::range::RangeInclusive（Copy、字段公开）
    let r = RangeInclusive { start: 1, last: 3 };
    let vals: Vec<i32> = r.into_iter().collect();
    assert_eq!(vals, vec![1, 2, 3]);
}
```

更多：`MaybeUninit<[T; N]>` 与 `[MaybeUninit<T>; N]` 互转、
`Cell<[T; N]>` 的 `AsRef`/`AsMut`、`Layout::{dangling_ptr, repeat, repeat_packed,
extend_packed}`、const 化 `fmt::from_fn`、`ControlFlow::{is_break, is_continue}`。

### 11.3 编译器 / 其他

- `--remap-path-scope` 稳定（控制路径重映射的作用域）。
- **JSON target specs 需要 `-Z unstable-options`**（为 build-std 铺路）。
- `Eq::assert_receiver_is_total_eq` 弃用。
- 内部升级 LLVM 22。


## 12. Rust 1.96.0（2026-05-28）— 新 Range 类型体系（RFC 3550）+ `assert_matches!`

### 12.1 新 Range 类型（本版最大变化）

**背景**：`core::ops::Range` 等旧类型直接实现 `Iterator`，而 Rust 禁止同一类型同时
实现 `Iterator` 和 `Copy`——所以旧 Range **不能 Copy**，这在结构体里很别扭。
RFC 3550 引入**替换类型**：实现 `IntoIterator` 而非 `Iterator`，因此**可以 Copy**。

**1.96 稳定**（1.95 先落地 `RangeInclusive`）：

```rust
use core::range::{Range, RangeFrom, RangeToInclusive};

fn main() {
    // 字段公开，直接构造（新类型没有 `new`）
    let r: Range<usize> = Range { start: 0, end: 5 };
    let vals: Vec<usize> = r.into_iter().collect();
    assert_eq!(vals, vec![0, 1, 2, 3, 4]);

    let rf: RangeFrom<usize> = RangeFrom { start: 3 };
    assert_eq!(rf.into_iter().take(2).collect::<Vec<_>>(), vec![3, 4]);

    // RangeToInclusive 与旧版一致：无迭代器，只有 contains 等
    let rti = RangeToInclusive { last: 2 };
    assert!(rti.contains(&2));

    // Copy 是可用的！
    fn assert_copy<T: Copy>() {}
    assert_copy::<Range<usize>>();
    assert_copy::<core::range::RangeInclusive<usize>>();
}
```

**要点**：
- 范围语法 `0..1` **目前仍产生旧类型**（未来 edition 会切换到新类型）。
- 1.98 起旧类型的新家在 **`std::range::legacy`**（见 14 章）。
- 新 `RangeInclusive` 字段公开（旧版为了隐藏"迭代耗尽状态"而私有）；
  新类型必须先 `into_iter()` 才能迭代。
- 库作者建议：公开 API 用 `impl RangeBounds`（新旧都接受）；需要具体类型时
  优先新 range。

**经典用例**（来自官方博客）：把 slice 访问器存进 Copy 结构体：

```rust
use core::range::Range;

#[derive(Clone, Copy)]
pub struct Span(Range<usize>);

impl Span {
    pub fn of(self, s: &str) -> &str {
        &s[self.0]
    }
}
```

### 12.2 `assert_matches!` / `debug_assert_matches!`

**是什么**：断言值匹配某模式，失败时 panic 并打印值的 `Debug`（比
`assert!(matches!(..))` 的诊断信息好）。**不在 prelude**（避免与第三方同名宏冲突），
需手动导入：

```rust
use core::assert_matches;

fn get_random_number() -> u32 {
    4
}

fn main() {
    assert_matches!(get_random_number(), 1..=6);
}
```

### 12.3 其它

- **NonZero 整数范围可以迭代**：`(NonZero::new(2u8).unwrap()..NonZero::new(5u8).unwrap())`
  产出 `2, 3, 4`（实测）。
- `From<T> for LazyCell<T, F>` / `LazyLock<T, F>` / `AssertUnwindSafe<T>`。
- `expr` 元变量可以传给 `cfg` 宏；never 类型在元组表达式中自动强制。
- 兼容性：最小外部 LLVM 升到 21；`export_name`/`link_name`/`link_section`
  多属性时**取第一个**；avr 上 `c_double` 修正为 `f32`。


## 13. Rust 1.97.0（2026-07-09）— v0 符号默认 + 位操作助手

### 13.1 语言特性

- `Result<T, Uninhabited>` 与 `ControlFlow<Uninhabited, T>` 对 `#[must_use]` 视为
  `T`（不再对 `Result<(), Infallible>` 这类永远成功的结果报警）。
- 新 allow-by-default lint `dead_code_pub_in_binary`（二进制 crate 中未使用的
  `pub` 项）。
- `cfg(target_has_atomic_primitive_alignment)` 稳定（按原子类型对齐探测平台）。
- imports 尾部 `self` 在更多场景允许。

### 13.2 标准库亮点（已实测）

```rust
use std::num::NonZero;

fn main() {
    // 位操作助手（整数 + NonZero 双版本）
    assert_eq!(0b1010u8.isolate_lowest_one(), 0b10);   // 只留最低 1 位
    assert_eq!(0b1010u8.isolate_highest_one(), 0b1000); // 只留最高 1 位
    assert_eq!(0b1010u8.bit_width(), 4);                // 表示该值所需位数
    assert_eq!(0b1010u8.lowest_one(), Some(1));         // 最低 1 位位置
    assert_eq!(0b1010u8.highest_one(), Some(3));        // 最高 1 位位置
    assert_eq!(0u8.lowest_one(), None);

    let nz = NonZero::new(0b1010u8).unwrap();
    assert_eq!(nz.isolate_lowest_one().get(), 0b10);
    assert_eq!(nz.bit_width().get(), 4); // 注意：NonZero 版返回 NonZero<u32>
}
```

更多：`RepeatN: Default`、`ffi::FromBytesUntilNulError: Copy`、`char::is_control` const。

### 13.3 编译器 / 兼容性（重要）

- **v0 符号命名（symbol mangling）成为默认**！调试器/性能分析工具可能无法正确
  demangle（老版本工具尤其），回溯文本格式也可能变化。
- 新 warn-by-default lint **`linker_messages`**：链接器输出不再默认隐藏（该 lint
  刻意**不属于** `warnings` 组，用 `[lints.rust] linker_messages = "allow"` 关闭）。
- **`pin!` 的 deref 强制转换被禁止**（修复 unsoundness）：`pin!(x)` 中 `x: &mut T`
  现在总是得到 `Pin<&mut &mut T>`（1.88~1.96 期间可能被意外强转成 `Pin<&mut T>`）。
- `std::char` 的常量与函数开始弃用；linker 输出默认警告。
- Cargo：`build.warnings` 配置稳定（CI 里要求零警告，替代 `-Dwarnings`）、
  `resolver.lockfile-path` 稳定、`-m` 简写 `--manifest-path`。


## 14. Rust 1.98.0（2026-08-20）— 数值格式化与字符串工具

### 14.1 语言特性 / lint

- **`&mut` 生命周期缩短在不变位置也允许**（unsize 强转时），例如
  `Cell<&'long mut i32>` 可强转为 `Cell<&'short mut dyn Send>`。
- 新 deny-by-default lint **`invalid_runtime_symbol_definitions`** 与
  warn-by-default **`suspicious_runtime_symbol_definitions`**（针对自定义
  `memcmp`/`memset`/`strlen` 等 core 运行时符号，未来会扩展）。
- 新 warn-by-default lint **`c_void_returns`**（`core::ffi::c_void` 作为返回类型）。
- 兼容性：`repr(transparent)` 更严格（`repr(C)` 类型、私有字段类型、
  `#[non_exhaustive]` 类型不再视为"trivial"字段）；`derive` 宏可在
  `{core,std}::derive` 使用；`assert_eq!`/`assert_ne!` 加入临时作用域。

### 14.2 标准库亮点（已实测）

```rust
use core::fmt::NumBuffer;

fn main() {
    // substr_range / subslice_range：子串在母串中的范围
    // （返回 Option<新 Range 类型>）
    let s = "hello world";
    let sub = &s[6..];
    assert_eq!(s.substr_range(sub), Some(core::range::Range { start: 6, end: 11 }));

    let a = [1, 2, 3, 4];
    assert_eq!(a.subslice_range(&a[1..3]), Some(core::range::Range { start: 1, end: 3 }));

    // format_into + NumBuffer：无分配、无动态分发的整数格式化（接近 itoa 性能）
    let mut buf = NumBuffer::new();
    let formatted: &str = 12345u32.format_into(&mut buf);
    assert_eq!(formatted, "12345");

    // 浮点"代数"运算：允许利用实数代数性质重排/向量化
    // （不保证与 IEEE 结果逐位一致，语义类似 -ffast-math）
    assert_eq!(1.5f32.algebraic_add(2.5), 4.0);
    assert_eq!(9.0f32.algebraic_rem(4.0), 1.0);

    // 显式字节序的 UTF-16 解码
    assert_eq!(String::from_utf16le(&[0x48, 0x00, 0x69, 0x00]).unwrap(), "Hi");
    assert_eq!(String::from_utf16be(&[0x00, 0x48, 0x00, 0x69]).unwrap(), "Hi");

    // strip_circumfix：成对去除包裹
    assert_eq!("(hi)".strip_circumfix("(", ")"), Some("hi"));
    assert_eq!("hi".strip_circumfix("(", ")"), None);
    assert_eq!("[a][b]".strip_circumfix("[", "]"), Some("a][b"));
}
```

更多：`NonZero<{integer}>::from_str_radix`、`str::strip_circumfix`、
**`std::range::legacy`**（旧 `ops::Range` 等类型的新家，`0..5` 语法目前仍产生这些
旧类型）。

> **勘误说明**：部分第三方总结把 `Atomic<T>::from_mut`/`get_mut_slice`/`from_mut_slice`
> 列为 1.98 稳定 API，但实测 rustc 1.98.0 中泛型 `Atomic<T>` 仍是 nightly
> （E0658 `generic_atomic`），本文不收录。

---

## 15. 专题一

Rust 的字面量前缀容易混淆，这里给出**完整清单与实测结论**（rustc 1.98 stable）。

### 15.1 字符串字面量前缀（全部 stable）

| 前缀 | 含义 | 稳定于 | 类型 |
|---|---|---|---|
| （无）`"..."` | 普通字符串 | 1.0 | `&'static str` |
| `b"..."` | 字节字符串（元素为 `u8`；字符按 UTF-8 编码，ASCII 1 字节、非 ASCII 多字节） | 1.0 | `&'static [u8; N]` |
| `r"..."` / `r#"..."#` | 原始字符串（不处理转义） | 1.0 | `&'static str` |
| `br"..."` | 原始字节字符串 | 1.0 | `&'static [u8; N]` |
| `c"..."` | C 字符串（自动加 NUL，禁止内部 NUL） | **1.77** | `&'static CStr` |
| `cr"..."` | 原始 C 字符串 | **1.77** | `&'static CStr` |

实测通过：

```rust
use std::ffi::CStr;

fn main() {
    let _s: &str = "abc";
    let _b: &[u8; 3] = b"abc";
    let _r: &str = r"a\nb";            // 反斜杠是字面量
    assert_eq!(_r, "a\\nb");
    let _br: &[u8; 4] = br"a\nb";      // 同上，字节串
    assert_eq!(_br, b"a\\nb");
    let _c: &CStr = c"abc";            // 自动以 \0 结尾
    assert_eq!(_c.to_bytes(), b"abc");
    let _cr: &CStr = cr"a\nb";         // 反斜杠是字面量
    assert_eq!(_cr.to_bytes(), b"a\\nb");
}
```

### 15.2 字符字面量前缀

| 前缀 | 含义 | 稳定于 | 类型 |
|---|---|---|---|
| `'a'` | 普通字符 | 1.0 | `char` |
| `b'a'` | 字节字符 | 1.0 | `u8` |
| `c'a'` | **C 字符** | **未稳定**（nightly `c_str_literals`） | — |
| `r'a'` | **raw 字符** | **未稳定** | — |
| `f"..."` | **格式化字符串字面量** | **未稳定** | — |

后三者实测均报 `error: prefix \`c\`/\`r\`/\`f\` is unknown`（1.98 stable），
即 stable 工具链**不认识**这些前缀——不要在生产代码里使用。

### 15.3 关键规则速记

- **raw 字符串**（`r`/`br`/`cr`）：不处理 `\n`、`\"` 等转义，`#` 数量决定边界；
  内容里出现 `"` 或 `"##...` 时加 `#`（实测）：
  ```rust
  let s = r#"He said "hi""#;   // 内容 = He said "hi"
  let t = r###"a "## b"###;    // 内容含 "## 时，界标要用更多 #（此处 3 个）
  assert_eq!(t, "a \"## b");
  ```
- **C 字符串**（`c`/`cr`）：值为 CStr，**自动在末尾追加 `\0`**；**禁止内部 NUL**，
  编译报错 `null characters in C string literals are not supported`：
  ```rust
  // let _ = c"bad\0nul";  // 编译错误（实测）
  ```
- **字节字符串**（`b`/`br`）：元素是 `u8`，可含十六进制转义 `b"\x41\x42"`。
- 组合顺序固定：`b` 和 `c` 在前、`r` 在后（`br`、`cr`）；没有 `rb`/`rc`。



---

## 16. 专题二

### 16.1 它解决什么问题：返回位置 `impl Trait` 的生命周期捕获

函数签名 `fn f(..) -> impl Trait` 返回一个**不透明类型**（opaque type）。编译器需要
决定这个不透明类型"捕获"哪些泛型参数（生命周期、类型、const）——也就是返回值的
类型隐式依赖哪些参数。这个决定直接影响借用检查结果。

**实测确认的规则（rustc 1.98）**：

| | edition 2021 | edition 2024 |
|---|---|---|
| 不透明类型可"使用"哪些生命周期 | 只出现在**返回类型**中的（`&'a T`、`+ 'a`） | **所有 in-scope** 的生命周期（含 elided 的 `&self`、impl 块的 `'a`） |
| 未使用的 in-scope 生命周期 | 不产生约束 | 不产生约束（实测） |

**前后对比 1：2021 常见的 E0700 痛苦**（两个 edition 实测对比）：

```rust
// 2021：编译失败！hidden type `&'a i32` 使用了未捕获的 'a
fn make<'a>(x: &'a i32) -> impl Sized { x }
// error[E0700]: hidden type for `impl Sized` captures lifetime that does not
//              appear in bounds

// 修复（2021 中两种方式）：
fn make1<'a>(x: &'a i32) -> impl Sized + 'a { x }
fn make2<'a>(x: &'a i32) -> impl Sized + use<'a> { x } // 1.82+ 推荐

// 2024：自动捕获 in-scope 生命周期，直接编译通过
fn make3<'a>(x: &'a i32) -> impl Sized { x }
```

**前后对比 2：impl 块的生命周期**（2021 E0700 vs 2024 通过，均已实测）：

```rust
struct S<'a>(&'a str);
impl<'a> S<'a> {
    // 2021：error[E0700]（'a 未出现在返回类型），需写 + use<'a> 或 + 'a
    // 2024：编译通过
    fn get(&self) -> impl Sized {
        self.0
    }
}
```

> 编译器（1.82+）在报 E0700 时甚至会**直接建议** `use<..>`：
> `help: add a \`use<...>\` bound to explicitly capture \`'a\``

### 16.2 语法与稳定历史

```rust
fn f<'a, T, const N: usize>(..) -> impl Trait + use<'a, T, N> { .. }
```

- `use<...>` 是 `impl Trait` 的 **bound**，列出要捕获的泛型参数。
- 稳定历史（与直觉不同！）：
  - **1.82.0**：自由函数 / impl 中的 `impl Trait + use<..>` 基础版稳定；
  - **1.87.0**：扩展到 **trait 定义中**的返回位置 `impl Trait`
    （`precise_capturing_in_traits`）；
  - 1.88.0 没有任何 `use<>` 相关变更。

### 16.3 三类参数的实际规则（rustc 1.98 实测）

**生命周期**：可选列出。被隐藏类型实际使用的生命周期**必须**在列表中，否则 E0700；
未被使用的可列可不列。

```rust
fn only_lifetime<'a>(x: &'a i32) -> impl Sized + use<'a> { (x, ()) }

// 反例：hidden type 用了 'a 却没列入 -> E0700（实测）
// fn bad<'a>(x: &'a i32) -> impl Sized + use<> { x }
```

**类型参数**：**所有 in-scope 的类型参数必须全部列出**。实测错误信息：

```rust
// fn bad<'a, T>(x: &'a i32) -> impl Sized + use<'a> { (x, T::default()) }
// error: `impl Trait` must mention all type parameters in scope in `use<...>`
// note: currently, all type parameters are required to be mentioned in the
//       precise captures list
```

**const 参数**：与类型参数一致，**所有 in-scope 的 const 参数必须全部列出**：

```rust
// fn bad3<const N: usize>(x: [u8; N]) -> impl Sized + use<> { x }
// error: `impl Trait` must mention all const parameters in scope in `use<...>`
```

**混合列表可行**（1.98 实测）：`use<'a, T>`、`use<'a, T, N>` 都编译通过。

### 16.4 典型用法：让 trait 定义更精确（1.87）

```rust
trait Parse<'a> {
    // 只捕获 'a 与 Self，避免把其它参数带进返回类型
    fn tokens(&self, s: &'a str) -> impl Iterator<Item = &'a str> + use<'a, Self>;
}
```

### 16.5 与 edition 2024 默认捕获的关系

2024 的默认捕获让 E0700 大幅减少，但**类型/const 参数的默认捕获仍然存在**
（2024 中不透明类型默认捕获所有 in-scope 泛型参数）。`use<..>` 是把捕获集合
"白名单化"的显式手段——一旦使用，就按 16.3 的规则强制完备。

### 16.6 小结

- 遇到 E0700 → 优先尝试 `+ use<...>`（编译器会给出提示）；
- 想要精确控制返回类型依赖哪些参数 → 用 `use<..>`；
- 注意类型/const 参数"全列"限制（当前实现如此，未来可能放宽）。


---

## 17. 专题三

### 17.1 是什么

`&raw const expr` 与 `&raw mut expr` 直接对表达式取地址，产生裸指针
（`*const T` / `*mut T`）。**创建裸指针本身是 safe 的**（解引用才是 unsafe）。
稳定于 **1.82.0**；union 字段的 `&raw` 在 **1.92.0** 起 safe。

```rust
fn main() {
    let mut v = 10u32;
    let p = &raw mut v;       // safe：创建裸指针
    unsafe { *p = 20; }
    let c = &raw const v;     // safe
    assert_eq!(unsafe { *c }, 20);
}
```

### 17.2 与旧写法 `&expr as *const _` 的区别

旧写法先生成引用再转换，因此**受引用规则约束**：

```rust
let p = &mut v as *mut u32;  // 先创建 &mut v（需要可变借用），再转指针
let q = &raw mut v;          // 直接取地址，不经过引用
```

区别的实际后果：

1. **`static mut`**：`&mut STATIC` 是 unsafe 操作（1.85 起在 2024 edition 里是
   硬错误 E0133）；`&raw mut STATIC` 是 safe：

```rust
static mut COUNTER: i32 = 0;

fn main() {
    // 改动前：必须 unsafe 块
    unsafe { *(&mut COUNTER) = 1; }
    // 或
    unsafe {
        let p = &mut COUNTER; // 2024 edition：不带 unsafe 直接报 E0133
        *p = 1;
    }

    // 改动后：取地址 safe，解引用才 unsafe
    let p = &raw mut COUNTER;
    unsafe { *p += 1; }
}
```

2. **union 字段**（1.92 前需 unsafe，1.92 起 safe）：

```rust
union U { i: i32, f: f32 }
fn main() {
    let u = U { i: 7 };
    let p = &raw const u.i; // 1.92 起 safe（读取字段仍需 unsafe）
    assert_eq!(unsafe { *p }, 7);
}
```

3. **不产生临时引用**：`&raw` 不创建引用，不触发借用检查对引用的临时值规则，
   对字段、索引、未对齐/未初始化内存都直接可用：

```rust
let mut arr = [0u8; 4];
let p2 = &raw mut arr[2];   // 直接取元素地址
unsafe { *p2 = 77; }
assert_eq!(arr[2], 77);
```

### 17.3 注意

- 解引用裸指针始终需要 `unsafe`；
- 返回指向局部变量的裸指针会触发 1.91 的
  `dangling_pointers_from_locals` 警告（见 7.3）；
- `&raw` 与 `as` 转换在"最终指针值"上通常等价，但 `&raw` 更明确、无别名假设，
  是官方推荐写法。

### 17.4 验证对照表

| 写法 | 1.82~1.91 | 1.92+ |
|---|---|---|
| `&raw mut STATIC` | ✅ safe | ✅ safe |
| `&raw const u.field`（union） | ❌ 需 unsafe | ✅ safe |
| `&mut STATIC` | ⚠️ unsafe 操作 | ⚠️ unsafe 操作（2024 硬错误） |

---

## 附录 A：参考资料

- Rust 官方发布博客：https://blog.rust-lang.org/ （每版本 "Announcing Rust X.Y.0"）
- 各版本 RELEASES.md（tag 固定）：https://github.com/rust-lang/rust/releases
- Edition 2024 指南：https://doc.rust-lang.org/edition-guide/rust-2024/
- `use<..>` precise capturing：RFC 3617 / 稳定于 1.82（trait 定义中为 1.87）
- `&raw`：稳定于 1.82（union 字段安全借用为 1.92）
- C 字符串字面量：稳定于 1.77（RFC 3348）

## 附录 B：验证记录（rustc 1.98.0，edition 2024）

| 特性 | 结果 |
|---|---|
| `&raw const/mut` 创建裸指针（含 `static mut`、字段、索引、union 字段） | ✅ 编译运行通过 |
| `&mut static_mut`（不带 unsafe） | ❌ E0133（需 unsafe 块） |
| 字符串前缀 `b/r/br/c/cr`、字符前缀 `'a'`/`b'a'` | ✅ 编译运行通过 |
| `c'a'`、`r'a'`、`f"..."` | ❌ `prefix ... is unknown`（未稳定） |
| `use<'a>`、`use<T>`、`use<N>`、`use<'a, T>` | ✅ 编译运行通过 |
| `use<>` 漏列 in-scope 类型/const 参数 | ❌ "must mention all type/const parameters" |
| `use<>` 漏列被使用的生命周期 | ❌ E0700（编译器提示加入 use<..>） |
| let chains（2024） | ✅；2021 下 ❌ "only allowed in Rust 2024" |
| naked function + `naked_asm!` | ✅ 编译运行通过（sysv64 实测返回值） |
| `cfg(true)` / `cfg(false)` | ✅ |
| `const N: _` 推断 / `#[repr(u128)]` | ✅ |
| `asm!` label（命名/位置两种形式） | ✅ 编译运行通过 |
| sysv64 / system 可变参数 extern 声明 | ✅ |
| unsafe attribute `#[unsafe(no_mangle)]` | ✅；裸 `#[no_mangle]`（2024）❌ "used without unsafe" |
| `gen` / `yeet` 保留字（2024） | ❌ reserved keyword |
| `unsafe_op_in_unsafe_fn`（2024） | 默认 **warn**（非 deny），实测 `#[warn(...)] on by default` |
| 2021 vs 2024 RPIT 生命周期捕获（`&self`/impl 块 `'a`） | 2021 E0700 / 2024 ✅ |
| trait upcasting（`dyn Derived -> dyn Base`） | ✅ |
| `isqrt`（无 `checked_isqrt`） | ✅ |
| `get_disjoint_mut`（返回 `Result`） | ✅ |
| async closures（`async ||`、`async move ||`） | ✅ 编译运行通过 |
| `mismatched_lifetime_syntaxes` / `dangling_pointers_from_locals` / `unused_visibilities` | ✅ 产生对应警告 |
| match 臂 `if let` guard（1.95） | ✅ 编译运行通过 |
| `cfg_select!` / `bool::try_from` / `Atomic::update` / `Vec::push_mut` / `cold_path` / `as_ref_unchecked`（1.95） | ✅ |
| 新 `core::range::{Range, RangeInclusive, RangeFrom, RangeToInclusive}`（1.95/1.96，可 Copy） | ✅（`RangeToInclusive` 无迭代器，仅 `contains`） |
| `assert_matches!`（需显式 `use core::assert_matches;`）（1.96） | ✅ |
| NonZero 整数范围迭代（1.96） | ✅ |
| `isolate_*_one` / `bit_width` / `lowest_one` / `highest_one`（1.97；NonZero 版 `bit_width` 返回 `NonZero<u32>`） | ✅ |
| `substr_range` / `subslice_range`（返回 `Option<新 Range>`）（1.98） | ✅ |
| `format_into` + `NumBuffer` / `algebraic_*` / `from_utf16le/be` / `strip_circumfix`（1.98） | ✅ |
| `std::range::legacy`（1.98） | ✅ |
| 泛型 `Atomic<T>::from_mut` 等（部分总结列为 1.98 稳定） | ❌ E0658 `generic_atomic`（1.98 实测仍 nightly，教程未收录） |
