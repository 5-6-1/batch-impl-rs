# batch-impl

为 Rust trait 批量生成 `impl` 块的过程宏库。

## 设计目标

batch-impl 的核心设计目标**不是委托**（delegate to inner type），而是**批量生成**（bulk generation）——把"为 N 个类型写 N 个 impl"压缩成一行声明式 DSL。

与生态中其他库的定位差异：

| | crates.io 的 `auto_impl` | `impl-tools` | `fortuples` | **batch-impl** |
|---|---|---|---|---|
| 定位 | 委托（wrapper → inner） | derive 替代 + 委托 | 仅元组 | **批量生成** |
| 代理类型 | `&, Box, Arc, Fn*` 等 8 种 | 任意（via Deref） | 元组 0..16 | **任意类型** |
| 泛型控制 | 自动推断 | 自动推断 | 自动 | **手动精确指定** |
| 自定义 body | ❌ | ❌ | 自动 | ✅ `{...}` |
| 元组生成 | ❌ | ❌ | `(), (A,)..(A..P)` | **`()^N` + 笛卡尔积** |
| `^` 运算符 | ❌ | ❌ | ❌ | ✅ |
| `-` 运算符 | ❌ | ❌ | ❌ | ✅ |
| `unsafe impl` | ❌ | ❌ | ❌ | ✅ |

## 安装

```toml
[dependencies]
batch-impl = "0.2.0"
```

需要 Rust 2021 edition 及以上。

## 两个入口

| 宏 | 用途 |
|---|---|
| `#[batch_impl]` | 属性宏，在 trait 定义上标注 |
| `batch_trait!` | 函数式宏，对已声明的 trait 批量生成 impl |

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

| 部分 | 示例 | 何时需要 |
|------|------|----------|
| `<impl-泛型>` | `<T>`, `<T: Clone>`, `<const N: usize>` | impl 块需要泛型参数时 |
| `Trait名<trait-泛型>` | `MyTrait<T>`, `MyTrait<Vec<T>>` | trait 定义有泛型参数时 |
| 目标类型 | `usize`, `Vec<T>`, `&str` | 必需 |
| `[...]` 列表 | `[A, B, C]` | 为多个类型同时实现 |
| `{ body }` | `{ fn m(&self) -> usize { 0 } }` | 需要自定义实现体时 |

## `^` 运算符（右结合）

`A^B^C = A^(B^C)`。左侧是"修饰符"，右侧是"目标类型"。修饰符可以是：

| 修饰符 | 含义 |
|--------|------|
| `&` | 引用 |
| `&mut` | 可变引用 |
| `*const` | 裸指针（不可变） |
| `*mut` | 裸指针（可变） |
| `self` | 恒等（不改变类型） |
| `unsafe` | 标记 impl 为 `unsafe impl` |
| `Ident` | 容器（如 `Box`, `Vec`） |
| `Ident<...>` | 带预填泛型的容器（如 `HashMap<K>`），`^` 追加参数 |
| `(A,)`/`(A,B)` | 元组前缀 |
| `()` | 空元组前缀 |
| `(<bound>)` | 带 trait bound 的泛型元组前缀 |
| `[A, B]` | 多修饰符（笛卡尔积展开） |

| 写法 | 展开 |
|------|------|
| `&^T` | `&T` |
| `&mut^T` | `&mut T` |
| `*const^T` | `*const T` |
| `*mut^T` | `*mut T` |
| `self^T` | `T` |
| `Box^T` | `Box<T>` |
| `Box^<X,Y>` | `Box<X, Y>`（多参容器） |
| `[Box, Vec]^T` | `Box<T>, Vec<T>` |
| `Box^[T1, T2]` | `Box<T1>, Box<T2>` |
| `[Box, Vec]^[T1, T2]` | 笛卡尔积共 4 项 |
| `Box^Box^T` | `Box<Box<T>>` |
| `HashMap<K>^V` | `HashMap<K, V>`（预填泛型追加） |
| `[HashMap<K>, Vec<K>]^V` | `HashMap<K, V>, Vec<K, V>` |
| `&^Box^T` | `&Box<T>`（引用类修饰符链式应用） |
| `*const^Vec^T` | `*const Vec<T>` |

## `-` 运算符（左结合）

`-` 与 `^` 语义完全相同，仅结合方向不同：`A-B = A^B`，`A-B-C = (A-B)-C`。

| 写法 | 展开 |
|------|------|
| `Vec-u32` | `Vec<u32>` |
| `HashMap-u32-String` | `HashMap<u32, String>`（左结合，预填泛型追加） |
| `()-[A, B]` | `(A,), (B,)` |
| `()-[A, B]-[C, D]` | `(A, C), (A, D), (B, C), (B, D)` |

## 元组生成

`^` 运算符右侧是数字时，生成指定长度的元组。

| 写法 | 展开 |
|------|------|
| `()^3` | `(A, B, C)`（带3个泛型参数） |
| `(T,)^3` | `(T, T, T)` |
| `(<Clone>)^3` | `(A:Clone, B:Clone, C:Clone)` |
| `(T1, T2)^2` | 笛卡尔积 `(T1,T1), (T1,T2), (T2,T1), (T2,T2)` |

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

语法：`Trait路径: impl-specs`，`;` 分隔多个 trait。`:` 前可加 `unsafe` 关键字。

## 设计决策

### 有意识不支持

- **where 子句**：不在 DSL 内。复杂 bound 写在 trait 定义本身
- **高阶 trait bound（`for<'a>`）**：where 子句式范畴；类型内部 token 透传，无需特殊处理
- **`TraitName<>`（空尖括号）**：视为"trait 无泛型"；无需指定时直接写 `TraitName`

### 歧义处理

- **`[]`**：有逗号是并列列表，无逗号是切片类型
- **`()`**：`()` = 空元组，`(A,)` = 单元素元组，`(A)` = 分组
- **`[<`**：Rust 的词法限制，`[<Trait>]` 中 `[<` 会被 token 化；拆成独立表达式使用

## 错误提示

宏对常见错误给出中文提示并指向源码位置：

| 错误输入 | 错误信息 |
|----------|----------|
| `#[batch_impl()]` | `batch_impl 至少需要一个类型参数` |
| `#[batch_impl(<T> MyTrait<T>)]` | `` `MyTrait<...>` 被解析为 trait 泛型参数，但缺少目标类型 `` |
| `#[batch_impl([])]` | `` 空的 `[]``` |
| `#[batch_impl(<T> Vec<T)]` | `未闭合的 <（有 1 层尖括号未关闭）` |
| `#[batch_impl((T)^3)]` | `` `(T)` 是分组而非元组，若需单元素元组请写 `(T,)` `` |

## 优先级

运算符优先级从高到低：

1. **`^`**（右结合）- 最高优先级
2. **`-`**（左结合）- 中等优先级
3. **`,`**（分隔符）- 最低优先级

示例：
- `A^B-C,D` = `(A^B)-C,D` = `(A<B>)-C,D` = `A<B,C>,D`
- `[A,B]^[C,D]-E` = `([A,B]^[C,D])-E` = `[A<C>,A<D>,B<C>,B<D>]-E`

## 内部原理

解析基于 `proc-macro2` 的 token tree 迭代器：

- **逗号切割**：仅按顶层 `<>` 深度判断；`()`、`[]`、`{}` 在 `proc-macro2` 中是 `Group` 令牌，内部逗号天然不暴露
- **泛型参数解析**：`parse_balanced` 做 `<` `>` 深度跟踪，支持任意嵌套类型、类型标注、const 泛型
- **目标类型**：原样透传为 `TokenStream`，不做解析——天然兼容一切合法 Rust 类型
- **span 传播**：错误信息和生成代码均保留原始输入位置
- **递归深度限制**：128 层上限防栈溢出
- **泛型名唯一化**：基于 Span 哈希后缀，多个 `()^N` 表达式不冲突

## 许可证

MIT OR Apache-2.0
