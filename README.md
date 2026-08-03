# batch-impl

为 Rust trait 批量生成 `impl` 块的过程宏库——**一行 DSL，展开成 N 个 impl**。

```rust
use batch_impl::batch_impl;
# use std::rc::Rc;

// 一个 body，为 4 种类型各生成一个 impl
#[batch_impl(<T> Sortable<T> [Box, Rc]^Vec<T> where{ T: Ord } {
    fn is_sorted(&self) -> bool { self.windows(2).all(|w| w[0] <= w[1]) }
})]
trait Sortable<T> { fn is_sorted(&self) -> bool; }
// → impl<T> Sortable<T> for Box<Vec<T>> where T: Ord { ... }
// → impl<T> Sortable<T> for Rc<Vec<T>>  where T: Ord { ... }

// 一行生成 4 个带泛型的元组 impl
#[batch_impl(()^4)]
trait TupleTrait {}
// → impl<A>       TupleTrait for (A,) {}
// → impl<A, B>    TupleTrait for (A, B) {}
// → impl<A, B, C> TupleTrait for (A, B, C) {}
// → impl<A, B, C, D> TupleTrait for (A, B, C, D) {}
```

## 核心心智模型

你写的是**一条"类型矩阵"的描述**，batch-impl 对矩阵的每个格子生成 impl：

```
#[batch_impl( <impl-泛型> Trait名<trait-泛型> 目标类型矩阵 { body }? )]
```

| 记号      | 含义                                  | 直觉                         |
|-----------|---------------------------------------|------------------------------|
| `^` / `-` | 应用：把左侧容器/修饰符作用到右侧类型 | **同一个运算**，仅结合性不同 |
| `[A, B]`  | 列表                                  | 横向展开（笛卡尔积）         |
| `(A, B)`  | 元组                                  | 排列（有序对）               |
| `#name`   | 指令：从 trait 定义自动抄 item 签名   | body 不用手写签名            |

`^` 与 `-` 是**同一运算**（左侧是修饰符/容器，右侧是目标类型），区别只在结合方向：

- `^` **右结合**，链式产生嵌套：`Box^Box^T` = `Box<Box<T>>`，`HashMap^K^V` = `HashMap<K<V>>`
- `-` **左结合**，链式累加参数：`HashMap-K-V` = `HashMap<K, V>`，`fn(A, B)-C` = `fn(A, B) -> C`

所以选哪个只看你想要的分组形状：想套娃用 `^`，想并列参数用 `-`。

`[A, B]^[X, Y]` = 2×2 矩阵（4 个 impl）；`(T1, T2)^2` = 排列（4 个有序对）。

## 快速开始

```toml
[dependencies]
batch-impl = "0.5.6"
```

需要 Rust 2024 edition 及以上。

```rust
use batch_impl::batch_impl;

// 1. 定义 trait，方法签名只写一次
trait Describe { fn describe(&self) -> String; }

// 2. 写一条 DSL：目标类型 + body（方法签名用 #fill 自动从 trait 抄）
#[batch_impl(
    [usize, isize] #fill(name){"number"},
    String #fill(name){"string"}
)]
trait Tagged { fn name(&self) -> &str; }
// → impl Tagged for usize  { fn name(&self) -> &str { "number" } }
// → impl Tagged for isize  { fn name(&self) -> &str { "number" } }
// → impl Tagged for String { fn name(&self) -> &str { "string" } }
```

## 语法参考

### spec 结构

| 部分                  | 示例                                    | 何时需要               |
|-----------------------|-----------------------------------------|------------------------|
| `<impl-泛型>`         | `<T>`, `<T: Clone>`, `<const N: usize>` | impl 块需要泛型参数时  |
| `Trait名<trait-泛型>` | `MyTrait<T>`, `MyTrait<Vec<T>>`         | trait 定义有泛型参数时 |
| 目标类型              | `usize`, `Vec<T>`, `&str`               | 必需                   |
| `[...]` 列表          | `[A, B, C]`                             | 为多个类型同时实现     |
| `{ body }`            | `{ fn m(&self) -> usize { 0 } }`        | 需要自定义实现体时     |

多个 spec 用 `,` 分隔：`#[batch_impl(usize, isize)]`。

> **泛型自动化只认同名**（trait 定义是唯一真相源）：
>
> - **`A<>` — trait 泛型照抄**：空实参列表表示"实参与 bound 全部来自 trait 定义"。
>   `trait Foo<T: Clone>` + `#[batch_impl(Foo<> ())]` 展开为
>   `impl<T: Clone> Foo<T> for ()`，一行都不用写泛型。仅 `#[batch_impl]` /
>   `#[batch_impl_only]` 可用（需要 trait 定义）；`batch_trait!` 无 trait 定义，
>   `A<>` 原样透传。
> - **`A<绑定们>` 同款照抄**：纯关联类型绑定（`A<Item=T>`，无位置参数）同样
>   照抄位置实参、绑定原样保留——`trait Foo<T: Clone> { type Item; }` +
>   `#[batch_impl(Foo<Item=T> ())]` → `impl<T: Clone> Foo<T> for () { type Item = T; }`。
>   含位置参数的 `A<T, Item=U>` 是普通 DSL 语法（不展开）。
> - **未写 bound 的同名继承**：`<T> Foo<T> Vec<T>` + `trait Foo<T: Clone>` 生成
>   `impl<T: Clone> Foo<T> for Vec<T>`——impl 参数按"在 trait 实参中的位置"对应
>   trait 形参，同名且未写 bound 时继承其内联 bound。
> - **trait 级 where 子句同款继承**：单一形参谓词
>   （`trait Foo<T> where T: Clone`）合并进 bound（内联 + where 拼接，
>   `A<>` 照抄同样带上）——`trait Foo<T: Clone> where T: Ord` 生成
>   `impl<T: Clone + Ord>`。复合谓词（`Vec<T>: Clone`、`Self: ...`）
>   保守跳过，请手写。
> - **改名 = 明确报错，绝不静默**：实参 `X` 对应形参 `T`（有 bound）但名字不同、
>   或继承的 bound 引用 `'a`/`U` 等形参名而 impl 未声明同名——均报
>   `compile_error!` 引导（请改名或手写 bound）。想用其他名字就手写 `<X: ...>`。
>
> 已写 bound 的参数宏不干预（`T: B` 是否蕴含 `T: Clone` 由 rustc 验证，
> 如 `trait B: A` 的父 trait 关系）。

### 运算符

DSL 通过四级优先级解析（从低到高）：

| 优先级 | 运算符 | 结合方向 | 说明                              |
|--------|--------|----------|-----------------------------------|
| 0      | `;`    | —        | `batch_trait!` 的段落分隔符       |
| 1      | `,`    | —        | impl-spec 列表分隔                |
| 2      | `-`    | 左结合   | 应用（同 `^` 语义，链式累加参数） |
| 3      | `^`    | 右结合   | 应用（链式嵌套）                  |

`(` `)` 分组在所有运算符之上起作用。

结合示例：

- `A^B-C,D` = `(A^B)-C,D` = `A<B,C>,D`
- `[A,B]^[C,D]-E` = `([A,B]^[C,D])-E` = `[A<C>,B<C>,A<D>,B<D>]-E`（数组-数组经逐层分发 + `expand` 摊平，产出 4 项；顺序不影响生成的 impl）
- `HashMap^K-V` = `(HashMap^K)-V` = `HashMap<K>-V` = `HashMap<K, V>`
- `fn^(A,B)-C` = `(fn^(A,B))-C` = `fn(A,B)->C`

> **注意**：`Box^Vec-u32` 是错误写法（会被解释为 `Box<Vec, u32>`），应写为 `Box^Vec^u32`。

> **操作数严格性**：`^`/`-`/`,` 两侧必须有操作数——`A^`、`^A`、`-A`、`,A`、`A,,B`
> 均报 `compile_error!`；仅**尾随逗号**（`A,` / `[A, B,]`）允许，`();`/`[]` 等
> 括号是真实 token 不算空操作数。`;` 作为 `batch_trait!` 段落边界保持宽松。

### `^` 修饰符

左侧的修饰符可以是：

| 修饰符         | 含义                                              |
|----------------|---------------------------------------------------|
| `&`            | 引用                                              |
| `&mut`         | 可变引用                                          |
| `*const`       | 裸指针（不可变）                                  |
| `*mut`         | 裸指针（可变）                                    |
| `self`         | 恒等（不改变类型）                                |
| `unsafe`       | 裸 `unsafe^T` 标记 impl 为 `unsafe impl`；`unsafe fn(...)` 则是 unsafe fn 类型 |
| `fn`           | 函数类型前缀                                      |
| `#[attr]`      | 属性前缀                                          |
| `Ident`        | 容器（如 `Box`, `Vec`）                           |
| `Ident<...>`   | 带预填泛型的容器（如 `HashMap<K>`），`^` 追加参数 |
| `(A,)`/`(A,B)` | 元组前缀                                          |
| `()`           | 空元组前缀                                        |
| `(<bound>)`    | 带 trait bound 的泛型元组前缀                     |
| `[A, B]`       | 多修饰符（笛卡尔积展开）                          |
| `[T]`          | 切片（`[T]^N` 填长度成定长数组）                  |
| `[]`           | 空基座（`[]^T` 包出切片，`[]-T-N` 造定长数组）    |

| 写法                     | 展开                              |
|--------------------------|-----------------------------------|
| `&^T`                    | `&T`                              |
| `&mut^T`                 | `&mut T`                          |
| `*const^T`               | `*const T`                        |
| `*mut^T`                 | `*mut T`                          |
| `self^T`                 | `T`                               |
| `Box^T`                  | `Box<T>`                          |
| `Box^<X,Y>`              | `Box<X, Y>`（多参容器）           |
| `[Box, Vec]^T`           | `Box<T>, Vec<T>`                  |
| `Box^[T1, T2]`           | `Box<T1>, Box<T2>`                |
| `[Box, Vec]^[T1, T2]`    | 笛卡尔积共 4 项                   |
| `Box^Box^T`              | `Box<Box<T>>`（右结合嵌套）       |
| `HashMap<K>^V`           | `HashMap<K, V>`（预填泛型追加）   |
| `[HashMap<K>, Vec<K>]^V` | `HashMap<K, V>, Vec<K, V>`        |
| `&^Box^T`                | `&Box<T>`（修饰符链式应用）       |
| `*const^Vec^T`           | `*const Vec<T>`                   |
| `fn^(A,B)`               | `fn(A,B)`（函数类型）             |
| `unsafe fn(A)->B`        | `unsafe fn(A)->B`（unsafe fn 类型）|
| `unsafe fn^(A,B)-C`      | `unsafe fn(A,B)->C`（unsafe fn 填参 + 返回） |
| `unsafe^T`               | `unsafe impl Trait for T`（impl 标记，`T` 可为普通类型） |
| `#[attr]^T`              | 在 impl 块前添加属性              |
| `[]^T`                   | `[T]`（空基座包出切片）           |
| `[T]^N`                  | `[T; N]`（定长数组，N 可为数字/const 泛型） |
| `[T]^1..3`               | `[T; 1], [T; 2]`（范围批量）      |
| `[T]^[1, 2, 4]`          | `[T; 1], [T; 2], [T; 4]`（指定长度） |

### `-` 运算符

与 `^` 同一运算，仅左结合（链式累加参数）：

| 写法                 | 展开                                           |
|----------------------|------------------------------------------------|
| `Vec-u32`            | `Vec<u32>`                                     |
| `HashMap-u32-String` | `HashMap<u32, String>`（左结合，参数累加）     |
| `()-[A, B]`          | `(A,), (B,)`                                   |
| `()-[A, B]-[C, D]`   | `(A, C), (A, D), (B, C), (B, D)`               |
| `[]-T-N`             | `[T; N]`（空基座 + 元素 + 长度 = 定长数组）     |

`[]` 作为 `-` 累加链的基座，可把整个类型矩阵包进 const 泛型定长数组：

```rust
# use batch_impl::batch_impl;
#[batch_impl(
    <const N: usize> []-[&, self, Box]^[u8, i8, ()^0..3]-N
)]
trait FixedMatrix {}
// → impl<const N: usize> FixedMatrix for [&u8; N]   { }
// → impl<const N: usize> FixedMatrix for [Box<i8>; N] { }
// → impl<const N: usize, A> FixedMatrix for [(A,); N] { }  // 元组 fresh 泛型自动外提
// → ...
```

### 元组生成

`^` 右侧是数字或范围时，生成指定长度的元组（数字只作为指数使用）：

| 写法          | 展开                                          |
|---------------|-----------------------------------------------|
| `()^3`        | `(A, B, C)`（带3个泛型参数）                  |
| `(T,)^3`      | `(T, T, T)`                                   |
| `(<Clone>)^3` | `(A:Clone, B:Clone, C:Clone)`                 |
| `(T1, T2)^2`  | 笛卡尔积 `(T1,T1), (T1,T2), (T2,T1), (T2,T2)` |
| `()^1..3`     | `(A,), (A, B)`（长度1到2）                    |
| `()^1..=3`    | `(A,), (A, B), (A, B, C)`（长度1到3）         |
| `(T,)^2..4`   | `(T, T), (T, T, T)`（长度2到3）               |

> 注意：`(T)` 是分组（非元组），`(T,)` 才是单元素元组。

### 歧义处理

- **`[]`**：有逗号是并列列表，无逗号是切片类型（如 `Box^[u32]` → `Box<[u32]>`）；空 `[]` 是数组/切片 builder 基座（`[]^T` → `[T]`，`[]-T-N` → `[T; N]`）
- **`[T]^N`**：切片填长度成定长数组（`[u8]^3` → `[u8; 3]`，`<const N: usize> [u8]^N` → `[u8; N]`）；`N` 仅限数字字面量、const 泛型标识符或列表/范围
- **`()`**：`()` = 空元组，`(A,)` = 单元素元组，`(A)` = 分组
- **`()^0`**：生成空元组 `()`，即 `impl Trait for ()`
- **`[T; N]`**：`[]` 内的 `;` 通过 DSL 的 `Semi` 优先级层级识别为定长数组分隔符

## 组合拳

### 共享 body + 列表

一个 body 为所有目标类型复用：

```rust
# use batch_impl::{batch_impl, batch_impl_only, batch_trait};
#[batch_impl([usize, isize, f32] {
    fn tag(&self) -> &'static str { "number" }
})]
trait Tagged { fn tag(&self) -> &'static str; }
```

### 嵌套泛型合并

列表项各自声明 impl 泛型，自动合并到 impl 块：

```rust
# use batch_impl::{batch_impl, batch_impl_only, batch_trait};
use std::collections::HashMap;

#[batch_impl(<T> Describe<T> [Vec<T>, <U> HashMap<T, U>] {
    fn describe(&self) -> String { format!("len={}", self.len()) }
})]
trait Describe<T> { fn describe(&self) -> String; }
// → impl<T>    Describe<T> for Vec<T>
// → impl<T, U> Describe<T> for HashMap<T, U>
```

### 独立/共享 body 合并

列表项可有独立 body，与共享 body 合并：

```rust
# use batch_impl::{batch_impl, batch_impl_only, batch_trait};
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

### 关联类型简洁写法

`Name=value` 语法在 trait 泛型参数中绑定关联类型：

```rust
# use batch_impl::{batch_impl, batch_impl_only, batch_trait};
#[batch_impl(<T> Iter<Item=T> Vec<T> {
    fn count(&self) -> usize { self.len() }
})]
trait Iter {
    type Item;
    fn count(&self) -> usize;
}
// → impl<T> Iter for Vec<T> { type Item = T; fn count(&self) -> usize { self.len() } }
```

支持多关联类型与泛型约束：

```rust
# use batch_impl::{batch_impl, batch_impl_only, batch_trait};
#[batch_impl(<T, U> Pair<First=T, Second=U> (T, U))]
trait Pair {
    type First;
    type Second;
}

#[batch_impl(<T: Clone> CloneIter<Item=T> Vec<T> {
    fn first(&self) -> T { self[0].clone() }
})]
trait CloneIter {
    type Item;
    fn first(&self) -> Self::Item;
}
```

### 指令系统

`#` 指令在预处理阶段展开，从 trait 定义自动读取 item 签名/类型，body 不用手写签名。

**`#name{body}` — 单 item 赋值**（fn / const / type 自动选择输出格式）：

```rust
# use batch_impl::{batch_impl, batch_impl_only, batch_trait};
#[batch_impl(usize #to_str{"usize"})]
trait ToString { fn to_str(&self) -> &str; }
// → impl ToString for usize { fn to_str(&self) -> &str { "usize" } }

#[batch_impl(usize #MAX_SIZE{1024})]
trait HasConst { const MAX_SIZE: usize; }
// → impl HasConst for usize { const MAX_SIZE: usize = 1024; }

#[batch_impl(usize #Item{u32})]
trait HasType { type Item; }
// → impl HasType for usize { type Item = u32; }
```

**`#fill(methods){body}` — 多方法同一 body**：

```rust
# use batch_impl::{batch_impl, batch_impl_only, batch_trait};
#[batch_impl(usize #fill(name, kind){"usize"})]
trait Describable { fn name(&self) -> &str; fn kind(&self) -> &str; }
// → 为 name 和 kind 各生成 { "usize" } body
```

特殊标记：`#all`（所有 item）、`#all_methods`（仅 fn）、`#all_constants`（仅 const）、`#all_types`（仅 type）。

```rust
# use batch_impl::{batch_impl, batch_impl_only, batch_trait};
#[batch_impl(usize #fill(#all){"default"})]
trait HasAll { fn method(&self) -> &str; const VALUE: &str; }
// → fn method 与 const VALUE 各生成 { "default" } body
```

**列表减法 `-name`**：参数中 `-` 前缀表示排除项（保留列表减去排除列表，排除优先）。
用于"批量实现除了某个 item 之外的所有项"：

```rust
# use batch_impl::{batch_impl, batch_impl_only, batch_trait};
#[batch_impl(usize #fill(#all,-skip_me){0})]
trait HasDefault {
    fn keep_me(&self) -> u32;
    fn skip_me(&self) -> u32 { 999 } // 默认实现，被排除后保留
    const VALUE: u32;
}
// → impl HasDefault for usize {
//       fn keep_me(&self) -> u32 { 0 }
//       const VALUE: u32 = 0;
//       // skip_me 不生成，走 trait 默认实现
//   }
```

`-` 后可跟标识符（`-foo`）或 `#all` 系列标记（`-#all_methods` = 排除所有方法）：
`#fill(#all,-#all_methods)` = 仅 const + type 项。也适用于 `#delegate`
（`#delegate(#all,-foo){target}`）。排除后为空、`-` 后缺目标会报 `compile_error!`。
`-` 只在指令参数域生效，与类型 DSL 的 `-` 连接运算符互不干扰。

**`#delegate(methods){target}` — 委托调用**：把方法委托到 target 表达式上调用同名方法。

```rust
# use batch_impl::{batch_impl, batch_impl_only, batch_trait};
// Vec<u32> 用 #name 提供 body，Box<Vec<u32>> 委托过去
#[batch_impl(
    Vec<u32> #d_len{self.len()},
    Box^Vec^u32 #delegate(d_len){**self}
)]
trait MyLen { fn d_len(&self) -> usize; }
// → impl MyLen for Box<Vec<u32>> { fn d_len(&self) -> usize { (**self).d_len() } }

// blanket impl 模式：具体类型 + 引用委托
#[batch_impl(i32 #to_i32{*self}, <T: ToI32> &T #delegate(to_i32){**self})]
trait ToI32 { fn to_i32(&self) -> i32; }
// → impl<T: ToI32> ToI32 for &T { fn to_i32(&self) -> i32 { (**self).to_i32() } }
```

### 指令与 DSL 组合

指令可与运算符、`{body}` 连续附着自由组合：

```rust
# use batch_impl::{batch_impl, batch_impl_only, batch_trait};
#[batch_impl(
    usize #name{"usize"} { fn kind(&self) -> &str { "number" } }
)]
trait Tagged { fn name(&self) -> &str; fn kind(&self) -> &str; }

#[batch_impl(<T: std::fmt::Display> Vec<T> #t10{self.len()})]
trait Len { fn t10(&self) -> usize; }
```

**扩展机制**：不认识的 `#name(args){body}` 自动转换为一个 `{...}` 代码块，内容是**函数式宏调用** `name!{(args){body} trait ...}`——把方法名列表、body 和整个 trait 定义一起交给用户的同名宏，由它展开为需要的 fn 定义。工作流程：预处理器遇到 `#my_handler(add,inc){*self+1}` → 不认识 `my_handler` → 展开为 `{my_handler!{(add,inc){*self+1} trait ...}}` → 附着到目标类型成为 impl body → 编译器在 impl 内展开 `my_handler!` 得到 fn 定义。这意味着指令系统是**开放的**，与 `#fill` / `#delegate` 完全同源：都是"读 trait → 生成 fn 定义"，只不过实现交给用户（`#fill` 是库实现，开放指令是用户宏实现）。

```rust
# use batch_impl::batch_impl;
# use batch_impl::batch_preprocess_test; // 测试用开放扩展宏：解析 (names){body} trait → 生成 fn 定义
#[batch_impl(usize #batch_preprocess_test(add,inc){*self+1})]
trait AddInc {
    fn add(&self) -> Self;
    fn inc(&self) -> Self;
}
// → trait AddInc { fn add(&self) -> Self; fn inc(&self) -> Self; }
// → impl AddInc for usize {
//       batch_preprocess_test!{(add,inc){*self+1} trait AddInc { fn add(&self) -> Self; fn inc(&self) -> Self; }}
//   }
//   → 宏展开为：fn add(&self) -> Self { *self + 1 } fn inc(&self) -> Self { *self + 1 }
```

> 说明：这是"用户自定义的 `#fill`"——每个类型可各挂一个（`usize #batch_preprocess_test(...){...}, isize #batch_preprocess_test(...){...}`），trait 定义仍只来自 `#[batch_impl]` 输出的 trait，不会重复。

### `where{...}` — where 子句

`where{...}` 后缀跟在目标类型之后，内是透传的 where 谓词；多个会合并：

```rust
# use batch_impl::{batch_impl, batch_impl_only, batch_trait};
#[batch_impl(<T: Clone> Sortable<T> Vec<T> where{ T: Ord } {
    fn sort(&self) -> Vec<T> { let mut v = self.clone(); v.sort(); v }
})]
trait Sortable<T> { fn sort(&self) -> Vec<T>; }
// → impl<T: Clone> Sortable<T> for Vec<T> where T: Ord { ... }

#[batch_impl(<A> <B> PairAB<A, B> (A, B) where{A: Clone} where{B: Clone} {
    fn pair(&self) -> (A, B) { (self.0.clone(), self.1.clone()) }
})]
trait PairAB<A, B> { fn pair(&self) -> (A, B); }
```

也支持 Rust 风格裸写 `where 谓词 {代码块}`（三个接口通用），谓词后的 `{...}` 代码块必须存在；谓词区边界为首个 `{...}` 代码块（`ident!{...}` 宏调用体与 `<N = {5}>` 尖括号内代码块不计入），逗号谓词不会被 spec 切分：

```rust
# use batch_impl::{batch_impl, batch_impl_only, batch_trait};
#[batch_impl(<A> <B> PairAB<A, B> (A, B) where A: Clone, B: Clone {
    fn pair(&self) -> (A, B) { (self.0.clone(), self.1.clone()) }
})]
trait PairAB<A, B> { fn pair(&self) -> (A, B); }
// → impl<A, B> PairAB<A, B> for (A, B) where A: Clone, B: Clone { ... }
```

多个 `where` 段可依次书写（`where A: Clone where B: Clone`），与旧式多 `where{...}` 等价。

### fn 类型

```rust
# use batch_impl::{batch_impl, batch_impl_only, batch_trait};
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

`unsafe fn(...)` 类型：`unsafe` 紧跟 `fn` 时修饰 fn 类型本身，与 `unsafe^T` 的
unsafe impl 标记无关（`unsafe^fn(...)` 才是"unsafe impl，目标为 fn 类型"）：

```rust
# use batch_impl::{batch_impl, batch_impl_only, batch_trait};
#[batch_impl(unsafe fn(i32, u32) -> u32)]
trait UnsafeFnType {}
// → impl UnsafeFnType for unsafe fn(i32, u32) -> u32 {}

#[batch_impl(unsafe fn^(i32, u32) - i64)]
trait UnsafeFnType2 {}
// → impl UnsafeFnType2 for unsafe fn(i32, u32) -> i64 {}
```

> **`unsafe` 歧义规则**：裸 `unsafe`（后跟 `^`/`-` 或单独出现）= unsafe impl 标记；
> `unsafe fn...` = unsafe fn 类型；`unsafe 其他类型`（并列、无运算符）= 报错
> （几乎必是忘写 `^` 的笔误，应写 `unsafe^T`）。

### unsafe / 指针 / 属性

```rust
# use batch_impl::{batch_impl, batch_impl_only, batch_trait};
#[batch_impl(unsafe^usize, isize)]
unsafe trait UnsafePartial {}
// unsafe trait 的所有 impl 自动 unsafe

#[batch_impl(*const^u32, *mut^i32)]
trait PtrMarker {}

#[batch_impl(*const^Box^u32)]
trait ConstPtrChain {}
// → impl ConstPtrChain for *const Box<u32> {}

#[batch_impl(#[allow(dead_code)]^usize, isize)]
trait AttrSimple {}
```

### 复杂类型透传

无法识别的类型原样透传：

```rust
# use batch_impl::{batch_impl, batch_impl_only, batch_trait};
#[batch_impl(
    (i32, String),
    &str,
    Box<dyn std::fmt::Display>,
    fn(i32) -> bool,
    dyn Fn() + Send + Sync
)]
trait ComplexMarker {}
```

## 三个入口

| 宏                   | 用途                                                     |
|----------------------|----------------------------------------------------------|
| `#[batch_impl]`      | 属性宏，在 trait 定义上标注，宏参数即 DSL                |
| `#[batch_impl_only]` | 同上，但丢弃 trait 定义，只输出 impl 块                  |
| `batch_trait!`       | 函数式宏，对已声明的 trait 批量生成 impl（支持多 trait） |

三者接受相同的 DSL 参数。

### `#[batch_impl_only]`

trait 已在别处定义、只需批量生成 impl 的场景。trait 定义仍要写出（只用来读取方法签名），输出不含 trait：

```rust
# use batch_impl::{batch_impl, batch_impl_only, batch_trait};
# trait Greet { fn hello(&self) -> &str; } // 真实 trait 在别处定义
#[batch_impl_only(usize #hello{"hi"})]
trait Greet { fn hello(&self) -> &str; } // 此 dummy 定义被丢弃
// → impl Greet for usize { fn hello(&self) -> &str { "hi" } }
```

支持 `#path::to::Trait:` 路径前缀，为外部模块中定义的 trait 生成 impl（路径末尾标识符必须与本地 dummy trait 名一致；`#[batch_impl]` 不支持此前缀）：

```ignore
// 路径前缀需要真实的外部模块上下文：doctest 的 fn main 内无法定义 pub 模块，
// 故此处仅示意语法（`mod` 是关键字不能作路径段，实际应为合法模块名）。
// 该特性的编译行为由 tests/regression.rs 中的路径 trait 用例覆盖。
#[batch_impl_only(#ext::traits::TraitName: usize, isize)]
trait TraitName { }
```

### `batch_trait!`

对已声明的 trait 批量生成 impl，`;` 分隔多个 trait 段。语法：`[unsafe] Trait路径: impl-specs`，接受与 `#[batch_impl]` 完全相同的 DSL 语法（`:` 右侧），额外支持多 trait 段、路径 trait（如 `foo::C`，见 tests/regression.rs）、unsafe 段：

```rust
use batch_impl::batch_trait;

trait A {}
trait B<T> {}
unsafe trait UnsafeTrait {}

batch_trait!(
    A: usize, isize;
    B: <T> B<T> Vec<T>;
    unsafe UnsafeTrait: usize
);
```

## 错误提示

所有 DSL 语法错误通过 `compile_error!()` 输出中文提示并指向源码位置，永不 panic：

| 错误输入               | 错误信息                                               |
|------------------------|--------------------------------------------------------|
| `batch_trait!(;)`      | `batch_trait! 中期望 trait 名称`                       |
| `batch_trait!(A)`      | `batch_trait! 中期望 ':' 分隔 trait 名称和 impl-specs` |
| `batch_trait!(A: B::)` | `batch_trait! 中期望标识符作为 trait 名称`             |
| 裸 `where` 缺代码块     | `batch-impl: \`where\` 谓词后缺少代码块 {...}`          |

## 内部架构

```text
lib.rs              宏入口（#[batch_impl] / #[batch_impl_only] / batch_trait!）+ TraitBounds / A<> 展开
  ├── batch_trait_entry.rs  共享驱动：BFS 展开并列列表 → 逐叶子 generate_impl
  ├── path_prefix.rs        外部 trait 路径前缀：#Path::to::Trait: 状态机解析
  ├── diagnostic.rs         统一 compile_error_str(msg) 用于编译期诊断
  ├── scan.rs               扫描与游标：Cursor<'a> + scan_stop（尖括号已配对，仅剩 -> 守卫）
  ├── parse/                解析层
  │   ├── mod.rs            DSL 解析器：优先级攀爬（Op::Semi/Comma/Dash/Caret/Prim）
  │   ├── parse_atom.rs     原子层解析：属性 / fn / 前缀 / 范围 / 分组 / 列表
  │   └── generic.rs        泛型解析：parse_generic / parse_angle_bracket_contents（尖括号组即 Delimiter::None 组）
  ├── preprocess/           预处理层
  │   ├── mod.rs            指令预处理：#name 指令展开（内置 + 开放扩展）
  │   ├── preprocess_helpers.rs  预处理辅助：build_from_item / get_trait_item / parse_names_from_tokens（列表减法 `-`）
  │   ├── where_process.rs  裸 where 改写：`where 谓词 {body}` → 旧式 `where{谓词}`
  │   └── angle.rs          尖括号组：入口 None 组扁平化 + `<...>` 配对为组（输出侧还原），parse 层不再管 <> 深度
  ├── ast/                  AST 层
  │   ├── mod.rs            Ty 枚举（18 个变体，含 Error）+ Op 优先级定义
  │   └── types_render.rs   AST 渲染：ToTokens impl for Ty + params_to_tokens 系列
  ├── apply/                运算层
  │   ├── mod.rs            Apply trait + 核心 apply() 两阶段分发（右操作数"结构"优先）
  │   └── apply_tuple.rs    元组与容器运算符 + 元组展开（^N / 笛卡尔积 / 范围 / fresh 泛型）
  └── codegen/
      └── mod.rs            代码生成：extract_impl_parts → hoist_type_params → generate_impl
```

解析流程：**token 流 → 指令预处理（每条指令展开为恰好一个 `{...}` 组）→ where 裸写改写 → Cursor 扫描取切片 → parse_item 优先级攀爬（`^`/`-` 经 `Apply` 组合：右操作数结构优先分发）→ Ty AST → 工作清单摊平并列列表 → 逐叶子 generate_impl**

### 错误处理

所有 DSL 语法错误均通过 `compile_error!()` 输出友好的编译错误，**永不 panic**。`Ty::Error` 变体在 apply/codegen 链路中透传，`preprocess` 层通过 `Result<_, TokenStream>` 传播，并由 `diagnostic.rs::compile_error_str` 统一构造 `compile_error!` token 流。

### 测试

测试矩阵分四层：

| 目录            | 文件             | 用途                                                                                                                                     |
|-----------------|------------------|------------------------------------------------------------------------------------------------------------------------------------------|
| `examples/`     | `quickstart.rs`  | 可运行的 DSL 主特性 demo（`cargo run --example quickstart`），14 段覆盖基础→复杂场景                                                      |
| `src/`          | `fuzz.rs`        | proptest 属性测试：随机 token 序列喂 `where_process` / `parse_item`，验证"不因用户输入 panic"（`cargo test --lib`）                       |
| `tests/`        | `dsl.rs`         | 33 个 `#[test]`，覆盖核心特性的语义回归（含 where 子句、外部路径前缀、宏调用边界、`unsafe fn` 类型、列表减法 `-`、`A<>` 与同名继承）                     |
| `tests/`        | `regression.rs`  | 23 个 `#[test]`，覆盖 dsl.rs 未触碰的 corner case：嵌套 `>>`、路径类型、const 泛型、生命周期、dyn + Send、路径前缀、数组/切片 builder、`batch_impl` vs `batch_trait!` 一致性 |
| `tests/`        | `ui.rs`          | `trybuild` UI 测试：22 个 `compile_fail` fixture 锁定诊断措辞 + 1 个 `pass` fixture                                                      |

运行：

```bash
cargo run --example quickstart       # 主特性 demo
cargo test --lib                     # 单元测试 + fuzz
cargo test --test dsl --test regression   # 功能与回归测试
cargo test --test ui                  # 诊断 UI 测试
# 重新生成 UI 快照：
TRYBUILD=overwrite cargo test --test ui
```

## 许可证

MIT OR Apache-2.0
