# batch-impl

为 Rust trait 批量生成 `impl` 块的过程宏库。

## 设计目标

batch-impl 的核心设计目标是**批量生成**（bulk generation）——把"为 N 个类型写 N 个 impl"压缩成一行声明式 DSL。

### 功能特性

| 特性            | 说明                                |
|---------------|-----------------------------------|
| 批量生成          | 为任意类型批量生成 impl 块                  |
| 泛型控制          | 手动精确指定 impl 泛型和 trait 泛型           |
| 自定义 body      | 每个类型可有独立的实现体                      |
| 元组生成          | `()^N` + 笛卡尔积 + 范围语法              |
| `^` 运算符       | 右结合，泛型应用和类型组合                     |
| `-` 运算符       | 左结合，与 `^` 语义相同                    |
| `unsafe impl` | 支持 unsafe impl 生成                  |
| 关联类型          | `Name=value` 语法绑定关联类型              |
| fn 类型         | 批量生成函数类型实现                         |
| 属性支持          | `#[...]` 语法为 impl 块添加属性            |
| `*const` / `*mut` | 裸指针类型                         |

## 安装

```toml
[dependencies]
batch-impl = "0.3.0"
```

需要 Rust 2024 edition 及以上。

## 两个入口

| 宏               | 用途                                |
|-----------------|-----------------------------------|
| `#[batch_impl]` | 属性宏，在 trait 定义上标注，宏参数即 DSL        |
| `batch_trait!`  | 函数式宏，对已声明的 trait 批量生成 impl（支持多 trait） |

两者接受相同的 DSL 参数。

## 快速开始

```rust
use batch_impl::batch_impl;

#[batch_impl(usize, isize)]
trait Numeric {}
// → impl Numeric for usize {}
// → impl Numeric for isize {}
```

## 语法概览

```
#[batch_impl( impl-spec [, impl-spec]* [ { body }]? )]
impl-spec = [ <impl-泛型> ] [ Trait名<trait-泛型> ] 目标 [ { body } ]
```

### 结构分解

| 部分                 | 示例                                      | 何时需要           |
|--------------------|-----------------------------------------|----------------|
| `<impl-泛型>`        | `<T>`, `<T: Clone>`, `<const N: usize>` | impl 块需要泛型参数时  |
| `Trait名<trait-泛型>` | `MyTrait<T>`, `MyTrait<Vec<T>>`         | trait 定义有泛型参数时 |
| 目标类型               | `usize`, `Vec<T>`, `&str`               | 必需             |
| `[...]` 列表         | `[A, B, C]`                             | 为多个类型同时实现      |
| `{ body }`         | `{ fn m(&self) -> usize { 0 } }`        | 需要自定义实现体时      |

## 运算符优先级

DSL 表达式通过四级运算符优先级解析（从低到高）：

| 优先级 | 运算符 | 结合方向 | 说明 |
|--------|--------|----------|------|
| 0 | `;` | — | `batch_trait!` 的段落分隔符 |
| 1 | `,` | — | impl-spec 列表分隔 |
| 2 | `-` | 左结合 | 泛型应用/类型组合（同 `^` 语义） |
| 3 | `^` | 右结合 | 泛型应用/类型组合 |

`(` `)` 分组在所有运算符之上起作用。

## `^` 运算符（右结合）

`A^B^C = A^(B^C)`。左侧是"修饰符"，右侧是"目标类型"。修饰符可以是：

| 修饰符            | 含义                                |
|----------------|-----------------------------------|
| `&`            | 引用                                |
| `&mut`         | 可变引用                              |
| `*const`       | 裸指针（不可变）                          |
| `*mut`         | 裸指针（可变）                           |
| `self`         | 恒等（不改变类型）                         |
| `unsafe`       | 标记 impl 为 `unsafe impl`           |
| `fn`           | 函数类型前缀                            |
| `#[attr]`      | 属性前缀                              |
| `Ident`        | 容器（如 `Box`, `Vec`）                |
| `Ident<...>`   | 带预填泛型的容器（如 `HashMap<K>`），`^` 追加参数 |
| `(A,)`/`(A,B)` | 元组前缀                              |
| `()`           | 空元组前缀                             |
| `(<bound>)`    | 带 trait bound 的泛型元组前缀             |
| `[A, B]`       | 多修饰符（笛卡尔积展开）                      |

| 写法                       | 展开                         |
|--------------------------|----------------------------|
| `&^T`                    | `&T`                       |
| `&mut^T`                 | `&mut T`                   |
| `*const^T`               | `*const T`                 |
| `*mut^T`                 | `*mut T`                   |
| `self^T`                 | `T`                        |
| `Box^T`                  | `Box<T>`                   |
| `Box^<X,Y>`              | `Box<X, Y>`（多参容器）          |
| `[Box, Vec]^T`           | `Box<T>, Vec<T>`           |
| `Box^[T1, T2]`           | `Box<T1>, Box<T2>`         |
| `[Box, Vec]^[T1, T2]`    | 笛卡尔积共 4 项                  |
| `Box^Box^T`              | `Box<Box<T>>`              |
| `HashMap<K>^V`           | `HashMap<K, V>`（预填泛型追加）    |
| `[HashMap<K>, Vec<K>]^V` | `HashMap<K, V>, Vec<K, V>` |
| `&^Box^T`                | `&Box<T>`（引用类修饰符链式应用）      |
| `*const^Vec^T`           | `*const Vec<T>`            |
| `fn^(A,B)`               | `fn(A,B)`（函数类型）            |
| `#[attr]^T`              | 在 impl 块前添加属性              |

## `-` 运算符（左结合）

`-` 与 `^` 语义完全相同，仅结合方向不同：`A-B = A^B`，`A-B-C = (A-B)-C`。

| 写法                   | 展开                                 |
|----------------------|------------------------------------|
| `Vec-u32`            | `Vec<u32>`                         |
| `HashMap-u32-String` | `HashMap<u32, String>`（左结合，预填泛型追加） |
| `()-[A, B]`          | `(A,), (B,)`                       |
| `()-[A, B]-[C, D]`   | `(A, C), (A, D), (B, C), (B, D)`   |

## 元组生成

`^` 运算符右侧是数字或范围时，生成指定长度的元组。

| 写法            | 展开                                        |
|---------------|-------------------------------------------|
| `()^3`        | `(A, B, C)`（带3个泛型参数）                      |
| `(T,)^3`      | `(T, T, T)`                               |
| `(<Clone>)^3` | `(A:Clone, B:Clone, C:Clone)`             |
| `(T1, T2)^2`  | 笛卡尔积 `(T1,T1), (T1,T2), (T2,T1), (T2,T2)` |
| `()^1..3`     | `(A,), (A, B)`（长度1到2）                     |
| `()^1..=3`    | `(A,), (A, B), (A, B, C)`（长度1到3）          |
| `(T,)^2..4`   | `(T, T), (T, T, T)`（长度2到3）                |

> 注意：`(T)` 是分组（非元组），`(T,)` 才是单元素元组。

## 使用示例

### 基础

```rust
use batch_impl::batch_impl;

#[batch_impl(usize, isize)]
trait Numeric {}

#[batch_impl(<T> Vec<T>)]
trait Collection {}
```

### Trait 带泛型参数

```rust
#[batch_impl(<T> FromValue<T> i32 {
    fn wrap(_val: T) -> Self { 0 }
})]
trait FromValue<T> { fn wrap(val: T) -> Self; }
```

### 并列列表 + 共享 body

```rust
#[batch_impl([usize, isize, f32] {
    fn tag(&self) -> &'static str { "number" }
})]
trait Tagged { fn tag(&self) -> &'static str; }
```

### 嵌套泛型合并

```rust
use std::collections::HashMap;

#[batch_impl(<T> Describe<T> [Vec<T>, <U> HashMap<T, U>] {
    fn describe(&self) -> String { format!("len={}", self.len()) }
})]
trait Describe<T> { fn describe(&self) -> String; }
// → impl<T>    Describe<T> for Vec<T>
// → impl<T, U> Describe<T> for HashMap<T, U>
```

### 关联类型简洁写法

在 trait 泛型参数中使用 `Name=value` 语法绑定关联类型：

```rust
#[batch_impl(<T> Iter<Item=T> Vec<T> {
    fn count(&self) -> usize { self.len() }
})]
trait Iter {
    type Item;
    fn count(&self) -> usize;
}
// → impl<T> Iter for Vec<T> { type Item = T; fn count(&self) -> usize { self.len() } }
```

支持多关联类型：

```rust
#[batch_impl(<T, U> Pair<First=T, Second=U> (T, U))]
trait Pair {
    type First;
    type Second;
}
```

支持泛型约束：

```rust
#[batch_impl(<T: Clone> CloneIter<Item=T> Vec<T> {
    fn first(&self) -> T { self[0].clone() }
})]
trait CloneIter {
    type Item;
    fn first(&self) -> Self::Item;
}
```

### 独立/共享 body 合并

列表项可有独立 body，与共享 body 合并：

```rust
#[batch_impl(
    [usize { fn name() -> &'static str { "usize" } },
     isize { fn name() -> &'static str { "isize" } }]
    { fn zero() -> Self { 0 } }
)]
trait Zero {
    fn zero() -> Self;
    fn name() -> &'static str;
}
// → impl Zero for usize { fn zero() -> Self { 0 } fn name() -> &'static str { "usize" } }
// → impl Zero for isize { fn zero() -> Self { 0 } fn name() -> &'static str { "isize" } }
```

纯独立 body（无共享）：

```rust
#[batch_impl(
    usize { fn describe(&self) -> String { format!("usize: {}", self) } },
    String { fn describe(&self) -> String { format!("string: {}", self) } }
)]
trait Describe {
    fn describe(&self) -> String;
}
```

### `^` 运算符

```rust
#[batch_impl([&, Box, Rc]^u32)]
trait RefOrOwned {}

#[batch_impl(HashMap^<u32, String>)]
trait MapMarker {}
```

### 元组生成

```rust
#[batch_impl(()^4)]
trait TupleTrait {}

#[batch_impl((<Clone>)^6)]
trait CloneTuple {}

// 范围语法
#[batch_impl(()^1..3)]
trait RangeTuple {}

#[batch_impl(()^1..=3)]
trait RangeIncTuple {}
```

### fn 类型

```rust
#[batch_impl(fn^(i32, u32))]
trait FnSimple {}

// fn 类型追加返回类型
#[batch_impl(fn(i32, u32)-String)]
trait FnWithReturn {}

// fn 类型批量生成（笛卡尔积）
#[batch_impl(fn-(i32, u32)^2)]
trait FnTupleGen {}
// → impl FnTupleGen for fn(i32, i32) {}
// → impl FnTupleGen for fn(i32, u32) {}
// → impl FnTupleGen for fn(u32, i32) {}
// → impl FnTupleGen for fn(u32, u32) {}
```

### unsafe

```rust
// 单个 spec 标记为 unsafe
#[batch_impl(unsafe^usize, isize)]
unsafe trait UnsafePartial {}

// unsafe trait 所有 impl 自动 unsafe
#[batch_impl(usize, Box<u32>)]
unsafe trait UnsafeAll {}
```

### 指针类型

```rust
#[batch_impl(*const^u32, *mut^i32)]
trait PtrMarker {}

// 指针链式应用
#[batch_impl(*const^Box^u32)]
trait ConstPtrChain {}
// → impl ConstPtrChain for *const Box<u32> {}
```

### 属性支持

```rust
#[batch_impl(#[allow(dead_code)]^usize, isize)]
trait AttrSimple {}
```

### 复杂类型透传

```rust
#[batch_impl(
    (i32, String),
    &str,
    Box<dyn std::fmt::Display>,
    fn(i32) -> bool,
    dyn Fn() + Send + Sync
)]
trait ComplexMarker {}
```

## `batch_trait!` 宏

对已声明的 trait 批量生成 impl。

```rust
use batch_impl::batch_trait;

trait A {}
trait B<T> {}
mod foo { pub trait C {} }

batch_trait!(
    A: usize, isize;
    B: <T> B<T> Vec<T>;
    foo::C: u32;
    unsafe UnsafeTrait: usize
);
```

语法：`[unsafe] Trait路径: impl-specs`，`;` 分隔多个 trait 段。

`batch_trait!` 接受与 `#[batch_impl]` 完全相同的 DSL 语法（`:` 右侧），额外支持：

- **多 trait**：以 `;` 分隔，每段可指定不同的 trait 路径
- **路径 trait**：支持 `mod::TraitName` 形式
- **unsafe 段**：`unsafe` 前缀标记该段所有 impl 为 unsafe impl

## 设计决策

### 有意识不支持

- **where 子句**：不在 DSL 内。复杂 bound 写在 trait 定义本身
- **高阶 trait bound（`for<'a>`）**：where 子句式范畴；类型内部 token 透传，无需特殊处理
- **`TraitName<>`（空尖括号）**：视为"trait 无泛型"；无需指定时直接写 `TraitName`
- **重复类型不去重**：`[usize, usize]` 会生成两个 impl（类型去重由用户负责）

### 歧义处理

- **`[]`**：有逗号是并列列表，无逗号是切片类型（如 `Box^[u32]` → `Box<[u32]>`）
- **`()`**：`()` = 空元组，`(A,)` = 单元素元组，`(A)` = 分组
- **`[<`**：Rust 的词法限制，`[<Trait>]` 中 `[<` 会被 token 化；拆成独立表达式使用
- **`()^0`**：生成空元组 `()`，即 `impl Trait for ()`
- **`[T; N]`**：`[]` 内的 `;` 通过 DSL 的 `Semi` 优先级层级识别为定长数组分隔符

## 错误提示

宏对常见错误给出中文提示并指向源码位置（`compile_error!`）：

| 错误输入                            | 错误信息                                         |
|---------------------------------|----------------------------------------------|
| `batch_trait!(;)`               | `batch_trait! 中期望 trait 名称`                    |
| `batch_trait!(A)`               | `batch_trait! 中期望 ':' 分隔 trait 名称和 impl-specs` |
| `batch_trait!(A: B::)`          | `batch_trait! 中期望标识符作为 trait 名称`              |

## 优先级

运算符优先级从高到低：

1. **`^`**（右结合）- 最高优先级
2. **`-`**（左结合）- 中等优先级
3. **`,`**（分隔符）- 最低优先级

示例：
- `A^B-C,D` = `(A^B)-C,D` = `(A<B>)-C,D` = `A<B,C>,D`
- `[A,B]^[C,D]-E` = `([A,B]^[C,D])-E` = `[A<C>,A<D>,B<C>,B<D>]-E`
- `HashMap^K-V` = `(HashMap^K)-V` = `HashMap<K>-V` = `HashMap<K, V>`
- `fn^(A,B)-C` = `(fn^(A,B))-C` = `fn(A,B)->C`

> **注意**：`Box^Vec-u32` 是错误写法（会被解释为Box<Vec,u32>），应写为 `Box^Vec^u32`。

## 内部架构

```
lib.rs          宏入口（#[batch_impl] / batch_trait!）
  ├── parse.rs    DSL 解析器：Cursor 游标 + 优先级攀爬（Op::Semi/Comma/Dash/Caret/Prim）
  ├── types.rs    AST 节点（Ty 枚举 + 20 个变体）+ Op 优先级定义
  ├── apply.rs    运算符语义：apply() 折叠规则 + 元组展开（^N / 笛卡尔积）
  └── codegen.rs  代码生成：Ty 递归拆解 → impl 块组装
```

解析流程：**token 流 → Cursor 扫描取切片 → parse_item 优先级攀爬 → Ty AST → BFS 展开并列列表 → 逐叶子 generate_impl**

## 许可证

MIT OR Apache-2.0
