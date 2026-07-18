# batch-impl

为 Rust trait 批量生成 `impl` 块的过程宏库。用轻量 DSL 描述「为哪些类型、在什么泛型条件下、以什么实现体」来实现 trait。

## 功能特性

- **批量生成 impl**：一个宏调用生成多个 impl 块
- **`^` 运算符**（右结合）：泛型应用，如 `Box^T` → `Box<T>`
- **`-` 运算符**（左结合）：同 `^`，如 `A-B` → `A<B>`
- **元组生成**：`()^3` → `(), (X,), (X,X)`
- **笛卡尔积**：`(T1,T2)^2` → `(T1,T1), (T1,T2), (T2,T1), (T2,T2)`
- **unsafe 支持**：`unsafe^T` 或 `unsafe trait` 自动生成 `unsafe impl`
- **batch_trait!**：对已声明的 trait 批量生成 impl

## 安装

```toml
[dependencies]
batch-impl = "0.1"
```

需要 Rust 2021 edition 及以上。

## 快速开始

```rust
use batch_impl::batch_impl;

#[batch_impl(usize, isize)]
trait Numeric {}
// 展开为：
// impl Numeric for usize {}
// impl Numeric for isize {}
```

## 语法概览

```
#[batch_impl( impl-spec [, impl-spec]* [ { body }]? )]
impl-spec = [ <impl-泛型> ] [ Trait名<trait-泛型> ] 目标 [ { body } ]
```

### 分解说明

| 部分 | 示例 | 何时需要 |
|------|------|----------|
| `<impl-泛型>` | `<T>`, `<T: Clone>`, `<const N: usize>` | impl 块需要泛型参数时 |
| `Trait名<trait-泛型>` | `MyTrait<T>`, `MyTrait<Vec<T>>` | trait 定义有泛型参数时 |
| `目标类型` | `usize`, `Vec<T>`, `&str` | 必需 |
| `[...]` 列表 | `[A, B, C]` | 为多个类型同时实现 |
| `{ body }` | `{ fn m(&self) -> usize { 0 } }` | 需要自定义实现体时 |

## `^` 运算符（右结合）

`A^B^C = A^(B^C)`

| 写法 | 展开 |
|------|------|
| `&^T` | `&T` |
| `&mut^T` | `&mut T` |
| `self^T` | `T` |
| `A^B` | `A<B>` |
| `A^<X,Y>` | `A<X,Y>` |
| `[A1,A2]^B` | `A1<B>, A2<B>` |
| `A^[B1,B2]` | `A<B1>, A<B2>` |
| `[A1,A2]^[B1,B2]` | 笛卡尔积 `A1<B1>, A1<B2>, A2<B1>, A2<B2>` |
| `Box^Box^T` | `Box<Box<T>>` |

## `-` 运算符（左结合）

`A-B = A^B`，`A-B-C = (A-B)-C`

| 写法 | 展开 |
|------|------|
| `()-[A,B]` | `(A,), (B,)` |
| `()-[A,B]-[C,D]` | `(A,C), (A,D), (B,C), (B,D)` |
| `()-[A]-[B]-[C]` | `(A,B,C)` |

## 元组生成

| 写法 | 展开 |
|------|------|
| `()^3` | `(), (X,), (X,X)` |
| `(T)^3` | `(), (T,), (T,T)` |
| `(<Clone>)^3` | `(), (A:Clone,), (A:Clone,B:Clone)` |
| `(T1,T2)^2` | 笛卡尔积 `(T1,T1), (T1,T2), (T2,T1), (T2,T2)` |
| `()^1..3` | `(X,), (X,X)` |
| `()^1..=3` | `(X,), (X,X), (X,X,X)` |

## 使用示例

### 基础用法

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
trait FromValue<T> {
    fn wrap(val: T) -> Self;
}
```

### 并列列表

```rust
#[batch_impl([usize, isize, f32] {
    fn tag(&self) -> &'static str { "number" }
})]
trait Tagged {
    fn tag(&self) -> &'static str;
}
```

### 嵌套泛型合并

```rust
use std::collections::HashMap;

#[batch_impl(<T> Describe<T> [Vec<T>, <U> HashMap<T, U>] {
    fn describe(&self) -> String { format!("len={}", self.len()) }
})]
trait Describe<T> {
    fn describe(&self) -> String;
}
// → impl<T>    Describe<T> for Vec<T>
// → impl<T, U> Describe<T> for HashMap<T, U>
```

### `^` 运算符

```rust
use batch_impl::batch_impl;

#[batch_impl(usize, isize)]
trait Numeric {}
```

### 元组生成

```rust
use batch_impl::batch_impl;

#[batch_impl(<T> Vec<T>)]
trait Collection {}
```

### unsafe 支持

```rust
// 单个 spec 标记为 unsafe
#[batch_impl(unsafe^usize, unsafe^Box<u32>, isize)]
unsafe trait UnsafePartial {}

// unsafe trait 所有 impl 自动 unsafe
#[batch_impl(usize, Box<u32>)]
unsafe trait UnsafeAll {}
```

### 复杂类型透传

```rust
#[batch_impl(
    (i32, String),
    &str,
    Box<dyn std::fmt::Display>,
    dyn Fn() + Send + Sync
)]
trait ComplexMarker {}
```

## `batch_trait!` 宏

对已声明的 trait 批量生成 impl 块。

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
// → impl A for usize {}
// → impl A for isize {}
// → impl<T> B<T> for Vec<T> {}
// → impl foo::C for u32 {}
// → unsafe impl UnsafeTrait for usize {}
```

语法：`Trait路径: impl-specs`，`;` 分隔多个 trait。

## 错误提示

宏会对常见错误给出中文提示并指向源码位置：

| 错误输入 | 错误信息 |
|----------|----------|
| `#[batch_impl()]` | `batch_impl 至少需要一个类型参数` |
| `#[batch_impl(<T> MyTrait<T>)]` | `` `MyTrait<...>` 被解析为 trait 泛型参数，但缺少目标类型 `` |
| `#[batch_impl([])]` | `` 空的 `[]`——若是并列列表请填入类型 `` |
| `#[batch_impl(<T> Vec<T)]` | `未闭合的 <（有 1 层尖括号未关闭）` |
| `#[batch_impl((T)^3)]` | `` `(T)` 是分组而非元组，若需单元素元组请写 `(T,)` `` |

## 设计约束

有意识不支持的特性：

- **where 子句**：不在 DSL 内。复杂的 trait bound 应写在 trait 定义本身
- **高阶 trait bound**（`for<'a>`）：已在 where 子句范畴内；出现在类型内部时走 token 透传，无需特殊处理
- **`TraitName<>`（空尖括号）**：被视为「trait 无泛型」；若不需要指定 trait 泛型，直接写 `TraitName` 即可

## 内部原理

解析基于 `proc-macro2` 的 token tree 迭代器：

- **逗号切割**：仅按顶层 `<>` 深度判断；`()`、`[]`、`{}` 在 `proc-macro2` 中是 `Group` 令牌，内部逗号天然不暴露
- **泛型参数解析**：用 `parse_balanced` 做 `<` `>` 深度跟踪，支持任意嵌套类型、类型标注、const 泛型
- **目标类型**：作为 `TokenStream` 原样透传，不做任何解析或校验——因此天然兼容一切合法 Rust 类型语法
- **span 传播**：错误信息和生成代码均保留原始输入位置，编译器报错指向用户源码
- **递归深度限制**：128 层上限防止栈溢出

## 许可证

MIT OR Apache-2.0
