# batch-impl 教程

渐进式学习 DSL：从一行 impl 开始，到高级矩阵组合。示例均为可编译代码，
每一步的产物都是普通 Rust——宏生成的 impl 与手写逐 token 等价。

## 1. 从一行 impl 开始

`#[batch_impl(...)]` 标注在 trait 定义上，参数里的每个 spec 生成一个 impl：

```rust
# use batch_impl::batch_impl;
#[batch_impl(usize, isize, f32, f64)]
trait Numeric {}
// → impl Numeric for usize {}
// → impl Numeric for isize {}
// → impl Numeric for f32 {}
// → impl Numeric for f64 {}
```

spec 的骨架：

```text
<impl-泛型> Trait名<trait-泛型> 目标类型 { body }?
```

| 部分                  | 示例                                    | 何时需要               |
|-----------------------|-----------------------------------------|------------------------|
| `<impl-泛型>`         | `<T>`, `<T: Clone>`, `<const N: usize>` | impl 块需要泛型参数时  |
| `Trait名<trait-泛型>` | `MyTrait<T>`, `MyTrait<Vec<T>>`         | trait 定义有泛型参数时 |
| 目标类型              | `usize`, `Vec<T>`, `&str`               | 必需                   |
| `{ body }`            | `{ fn m(&self) -> usize { 0 } }`        | 需要自定义实现体时     |

多个 spec 用 `,` 分隔：`#[batch_impl(usize, isize)]`。

## 2. 列表与 body

### 并列列表 `[A, B]`

一个 body 为所有目标类型复用：

```rust
# use batch_impl::batch_impl;
#[batch_impl([usize, isize, f32] {
    fn tag(&self) -> &'static str { "number" }
})]
trait Tagged { fn tag(&self) -> &'static str; }
// → impl Tagged for usize { fn tag(&self) -> &'static str { "number" } }
// → impl Tagged for isize { ... }
// → impl Tagged for f32   { ... }
```

### 独立/共享 body 合并

列表项可有独立 body，与共享 body 合并：

```rust
# use batch_impl::batch_impl;
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

## 3. 运算符 `^` 与 `-`

`^` 与 `-` 是**同一运算**：左侧是修饰符/容器，右侧是目标类型。区别只在结合性：
`^` 右结合（嵌套），`-` 左结合（累加参数）。

优先级从低到高：`;` < `,` < `-` < `^`，`()` 分组在所有运算符之上。

| 写法                     | 展开                                 |
|--------------------------|--------------------------------------|
| `Box^T`                  | `Box<T>`                             |
| `Box^<X,Y>`              | `Box<X, Y>`（多参容器）              |
| `Box^Box^T`              | `Box<Box<T>>`（右结合嵌套）          |
| `HashMap<K>^V`           | `HashMap<K, V>`（预填泛型追加）      |
| `&^Box^T`                | `&Box<T>`（修饰符链式应用）          |
| `Vec-u32`                | `Vec<u32>`                           |
| `HashMap-u32-String`     | `HashMap<u32, String>`（左结合累加） |
| `fn^(A,B)-C`             | `fn(A,B)->C`                         |
| `[Box, Vec]^T`           | `Box<T>, Vec<T>`                     |
| `Box^[T1, T2]`           | `Box<T1>, Box<T2>`                   |
| `[Box, Vec]^[T1, T2]`    | 笛卡尔积共 4 项                      |
| `[HashMap<K>, Vec<K>]^V` | `HashMap<K, V>, Vec<K, V>`           |

> **注意**：`Box^Vec-u32` 是错误写法（会被解释为 `Box<Vec, u32>`），应写为 `Box^Vec^u32`。

> **操作数严格性**：`^`/`-`/`,` 两侧必须有操作数——`A^`、`^A`、`-A`、`,A`、`A,,B`
> 均报 `compile_error!`；仅**尾随逗号**（`A,` / `[A, B,]`）允许，`();`/`[]` 等
> 括号是真实 token 不算空操作数。`;` 作为 `batch_trait!` 段落边界保持宽松。

## 4. 泛型声明

```rust
# use batch_impl::batch_impl;
#[batch_impl(<T> Vec<T>)]
trait Collection {}
// → impl<T> Collection for Vec<T> {}
```

### 嵌套泛型合并

列表项各自声明 impl 泛型，自动合并到 impl 块：

```rust
# use batch_impl::batch_impl;
# use std::collections::HashMap;
#[batch_impl(<T> Describe<T> [Vec<T>, <U> HashMap<T, U>] {
    fn describe(&self) -> String { format!("len={}", self.len()) }
})]
trait Describe<T> { fn describe(&self) -> String; }
// → impl<T>    Describe<T> for Vec<T>
// → impl<T, U> Describe<T> for HashMap<T, U>
```

### const 泛型

```rust
# use batch_impl::batch_impl;
#[batch_impl(<const N: usize> ConstGeneric<N> [i32; N] {
    fn len_const(&self) -> usize { N }
})]
trait ConstGeneric<const N: usize> { fn len_const(&self) -> usize; }
// → impl<const N: usize> ConstGeneric<N> for [i32; N] { ... }
```

## 5. 泛型自动化（trait 定义是唯一真相源）

### `A<>` — trait 泛型照抄

空实参列表表示"实参与 bound 全部来自 trait 定义"：

```rust
# use batch_impl::batch_impl;
#[batch_impl(Foo<> ())]
trait Foo<T: Clone> {}
// → impl<T: Clone> Foo<T> for ()
```

仅 `#[batch_impl]` / `#[batch_impl_only]` 可用（需要 trait 定义）；
`batch_trait!` 无 trait 定义，`A<>` 原样透传。

### `A<绑定们>` — 同款照抄

纯关联类型绑定（`A<Item=T>`，无位置参数）同样照抄位置实参、绑定原样保留：

```rust
# use batch_impl::batch_impl;
#[batch_impl(Foo<Item=T> ())]
trait Foo<T: Clone> { type Item; }
// → impl<T: Clone> Foo<T> for () { type Item = T; }
```

含位置参数的 `A<T, Item=U>` 是普通 DSL 语法（不展开）。

### 未写 bound 的同名继承

impl 参数按"在 trait 实参中的位置"对应 trait 形参，同名且未写 bound 时继承：

```rust
# use batch_impl::batch_impl;
#[batch_impl(<T> Foo<T> Vec<T> { fn get(&self) -> T { self[0].clone() } })]
trait Foo<T: Clone> { fn get(&self) -> T; }
// → impl<T: Clone> Foo<T> for Vec<T> { ... }
```

生命周期 bound（`<'a, T>` + `trait Foo<'a, T: 'a>` → `impl<'a, T: 'a>`）、
`'static`、混合 bound（`Clone + 'a`）一并继承。

### trait 级 where 子句继承

`trait Foo<T> where T: Clone` 的谓词**全形态继承**：

```rust
# use batch_impl::batch_impl;
#[batch_impl(<T> Foo<T> ())]
trait Foo<T: Clone>
where
    T: Ord,
{
}
// → impl<T: Clone + Ord> Foo<T> for ()
```

- **单一形参谓词**（`T: Clone`）合并进 bound（内联 + where 拼接），`<T>` 与
  `A<>` 两种写法同效；
- **其余谓词原样透传**到 impl 的 where 子句：`T::Item: Clone`、`Vec<T>: ...`、
  生命周期谓词（`'a: 'b`）等全部覆盖。

```rust
# use batch_impl::batch_impl;
#[batch_impl(<T> Foo<T> ())]
trait Foo<T>
where
    T: IntoIterator,
    T::Item: Clone,
{
}
// → impl<T: IntoIterator> Foo<T> for () where T::Item: Clone
```

### 改名 = 明确报错，绝不静默

实参 `X` 对应形参 `T`（有 bound）但名字不同、或继承的 bound/谓词引用
`'a`/`U` 等形参名而 impl 未声明同名——均报 `compile_error!` 引导
（请改名或手写 bound）。想用其他名字就手写 `<X: ...>`。

已写 bound 的参数宏不干预（`T: B` 是否蕴含 `T: Clone` 由 rustc 验证，
如 `trait B: A` 的父 trait 关系）。

## 6. 关联类型简洁写法

`Name=value` 语法在 trait 泛型参数中绑定关联类型：

```rust
# use batch_impl::batch_impl;
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
# use batch_impl::batch_impl;
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

## 7. 指令系统

`#` 指令在预处理阶段展开，从 trait 定义自动读取 item 签名/类型，body 不用手写签名。

### `#name{body}` — 单 item 赋值（fn / const / type 自动选择输出格式）

```rust
# use batch_impl::batch_impl;
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

### `#fill(methods){body}` — 多方法同一 body

```rust
# use batch_impl::batch_impl;
#[batch_impl(usize #fill(name, kind){"usize"})]
trait Describable { fn name(&self) -> &str; fn kind(&self) -> &str; }
// → 为 name 和 kind 各生成 { "usize" } body
```

特殊标记：`#all`（所有 item）、`#all_methods`（仅 fn）、`#all_constants`（仅 const）、`#all_types`（仅 type）。

```rust
# use batch_impl::batch_impl;
#[batch_impl(usize #fill(#all){"default"})]
trait HasAll { fn method(&self) -> &str; const VALUE: &str; }
// → fn method 与 const VALUE 各生成 { "default" } body
```

### 列表减法 `-name`

参数中 `-` 前缀表示排除项（保留列表减去排除列表，排除优先）。
用于"批量实现除了某个 item 之外的所有项"：

```rust
# use batch_impl::batch_impl;
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

### `#delegate(methods){target}` — 委托调用

把方法委托到 target 表达式上调用同名方法：

```rust
# use batch_impl::batch_impl;
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
# use batch_impl::batch_impl;
#[batch_impl(
    usize #name{"usize"} { fn kind(&self) -> &str { "number" } }
)]
trait Tagged { fn name(&self) -> &str; fn kind(&self) -> &str; }

#[batch_impl(<T: std::fmt::Display> Vec<T> #t10{self.len()})]
trait Len { fn t10(&self) -> usize; }
```

### 扩展机制（开放指令系统）

不认识的 `#name(args){body}` 自动转换为一个 `{...}` 代码块，内容是**函数式宏调用**
`name!{(args){body} trait ...}`——把方法名列表、body 和整个 trait 定义一起交给
用户的同名宏，由它展开为需要的 fn 定义。这意味着指令系统是**开放的**，与
`#fill` / `#delegate` 完全同源：都是"读 trait → 生成 fn 定义"，只不过实现交给
用户（`#fill` 是库实现，开放指令是用户宏实现）。

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

> 说明：这是"用户自定义的 `#fill`"——每个类型可各挂一个
> （`usize #batch_preprocess_test(...){...}, isize #batch_preprocess_test(...){...}`），
> trait 定义仍只来自 `#[batch_impl]` 输出的 trait，不会重复。

### `#blanket(methods){包装列表}` — 覆盖式委托

为包装类型批量生成委托 impl：`{包装列表}` 里的每个元素**可以是任意类型
表达式**（`&` / `&mut` / `Box` / `Rc` / `Arc` / `MyPtr` / `Box^Arc` /
`Cow<'_>`…），各生成一段完整委托 spec。先给内部类型实现 trait，再 blanket
覆盖包装：

```rust
# use batch_impl::batch_impl;
# use std::rc::Rc;
#[batch_impl(u32 { fn name(&self) -> String { self.to_string() } })]
#[batch_impl(#blanket(#all){&, Box, Rc})]
trait Name {
    fn name(&self) -> String;
}
// → impl Name for u32 { ... }                       // 第一个 batch_impl
// → impl<T: Name> Name for &T    { fn name(&self) -> String { (**self).name() } }
// → impl<T: Name> Name for Box<T> { ... }           // blanket 各包装一段委托
// → impl<T: Name> Name for Rc<T>  { ... }
```

**嵌套包装用 `^` 链**（目标类型 = 包装表达式 `^T`，T 为 fresh 泛型），
`<` 预填是追加语义（`Box<Arc>^T` = `Box<Arc, T>`，错误）：
`Box^Arc:2` → `Box<Arc<T>>`；`Cow<'_>` → `Cow<'_, T>`。

**委托体解引用层数**：默认 1（`**self`）；嵌套须显式 `:N`（`*` 数量 =
N + 1，如 `Box^Arc:2` → `***self`）。宏不猜包装内部的 Deref 层数——嵌套
包装忘标 `:N` 会退化为 rustc 方法不存在错误。

```rust
# use batch_impl::batch_impl;
# use std::rc::Rc;
#[batch_impl(u32 { fn deep(&self) -> u32 { *self } })]
#[batch_impl(#blanket(deep){Box^Rc:2, Box^Box^Box:3})]
trait Deep {
    fn deep(&self) -> u32;
}
```

`methods` 与 `#delegate` 相同（`#all` / `#all_methods` / 显式方法名列表）。

**泛型 trait 支持**（`trait Foo<X: Clone>`）：trait 形参照抄为 impl 泛型
（`impl<X: Clone, T: Foo<X>> Foo<X> for 包装<T> where ...`），trait 级
where 谓词透传。

**assoc type / const 委托**：`#all` 含 const/type 项时生成投影
`type Item = <T as Foo<X>>::Item;` / `const N: Ty = <T as Foo<X>>::N;`——
带必需关联类型的 trait 也能 blanket 覆盖。

```rust
# use batch_impl::batch_impl;
#[batch_impl(Foo<u32> u32 {
    type Item = u8;
    fn m(&self) -> u32 { *self }
})]
#[batch_impl(#blanket(#all){&, Box})]
trait Foo<X: Clone> {
    type Item;
    fn m(&self) -> X;
}
// → impl<X: Clone, T: Foo<X>> Foo<X> for Box<T> {
//     type Item = <T as Foo<X>>::Item;
//     fn m(&self) -> X { (**self).m() }
//   }
```

约束：`*const`/`*mut`（安全代码无法解引用裸指针委托）、`self`（无意义）、
空元素 / 非法 `:N` 均报错，请手写 `#delegate`。by-value receiver 方法
（`fn consume(self)`）委托语义取决于包装的 Deref/move 能力，宏展开期无法
区分——维持全放行，由 rustc 兜底。

## 8. where 子句

### `where{...}` 后缀

`where{...}` 后缀跟在目标类型之后，内是透传的 where 谓词；多个会合并：

```rust
# use batch_impl::batch_impl;
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

### 裸写 `where 谓词 {代码块}`

也支持 Rust 风格裸写（三个接口通用），谓词后的 `{...}` 代码块必须存在；
谓词区边界为首个 `{...}` 代码块（`ident!{...}` 宏调用体与 `<N = {5}>` 尖括号内
代码块不计入），逗号谓词不会被 spec 切分：

```rust
# use batch_impl::batch_impl;
#[batch_impl(<A> <B> PairAB<A, B> (A, B) where A: Clone, B: Clone {
    fn pair(&self) -> (A, B) { (self.0.clone(), self.1.clone()) }
})]
trait PairAB<A, B> { fn pair(&self) -> (A, B); }
// → impl<A, B> PairAB<A, B> for (A, B) where A: Clone, B: Clone { ... }
```

多个 `where` 段可依次书写（`where A: Clone where B: Clone`），与旧式多 `where{...}` 等价。

## 9. fn 类型

```rust
# use batch_impl::batch_impl;
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
# use batch_impl::batch_impl;
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

## 10. 修饰符大全

| 修饰符    | 含义                                          |
|-----------|-----------------------------------------------|
| `&`       | 引用（`&^T` → `&T`）                          |
| `&mut`    | 可变引用（`&mut^T` → `&mut T`）               |
| `*const`  | 裸指针（`*const^T` → `*const T`）             |
| `*mut`    | 可变裸指针（`*mut^T` → `*mut T`）             |
| `self`    | 恒等（`self^T` → `T`）                        |
| `unsafe`  | 裸 `unsafe^T` = unsafe impl 标记              |
| `#[attr]` | 属性前缀（`#[attr]^T` → impl 前加属性）       |
| `[]`      | 空基座（`[]^T` → `[T]`，`[]-T-N` → `[T; N]`） |
| `[T]`     | 切片（`[T]^N` → 定长数组 `[T; N]`）           |

```rust
# use batch_impl::batch_impl;
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

### 数组/切片 builder

```rust
# use batch_impl::batch_impl;
#[batch_impl([]^u8)]          // → impl ArrSlice for [u8] {}
trait ArrSlice {}

#[batch_impl([u8]^3)]         // → impl ArrLit for [u8; 3] {}
trait ArrLit {}

#[batch_impl(<const N: usize> [u8]^N)]  // → impl<const N: usize> ArrConst for [u8; N] {}
trait ArrConst {}

#[batch_impl([u8]^1..3)]      // → impl ArrRange for [u8; 1] {} 与 [u8; 2] {}
trait ArrRange {}
```

### 复杂类型透传

无法识别的类型原样透传：

```rust
# use batch_impl::batch_impl;
#[batch_impl(
    (i32, String),
    &str,
    Box<dyn std::fmt::Display>,
    fn(i32) -> bool,
    dyn Fn() + Send + Sync
)]
trait ComplexMarker {}
```

## 11. 元组生成与矩阵

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

```rust
# use batch_impl::batch_impl;
#[batch_impl(()^1..=4 { fn describe(&self) -> &'static str { "tuple" } })]
trait DescribeTuple { fn describe(&self) -> &'static str; }
// → 4 个 impl：(A,)、(A, B)、(A, B, C)、(A, B, C, D)
```

### 把整个矩阵包进 const 泛型定长数组

`[]` 作为 `-` 累加链的基座：

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

### `@` 常量 — 内置类型族命名

常用类型矩阵不必手写：`@` 常量在预处理阶段展开为字面列表，与手写等价。

| 常量 | 展开 |
|------|------|
| `@uint` | `[u8, u16, u32, u64, u128, usize]` |
| `@int` | `[i8, i16, i32, i64, i128, isize]` |
| `@float` | `[f32, f64]` |
| `@num` | `@uint + @int + @float`（14 个） |
| `@scalar` | `@num + [bool, char]`（16 个） |
| `@u8..u128` | `[u8, u16, u32, u64, u128]`（**含端点**；`@i8..i128` / `@f32..f64` 同款） |

```rust
# use batch_impl::batch_impl;
#[batch_impl(@scalar)]
trait ScalarTrait {}
// → 16 个 impl：u8..char 各一个
```

三个入口（`#[batch_impl]` / `#[batch_impl_only]` / `batch_trait!`）都支持内置
常量。`batch_trait!` 额外支持**自定义常量**：宏参数前导 `@name=值;` 段定义，
后续段落复用。值是**任意 token**（**懒展开**——原样入库，引用处拼接后递归
展开），因此值里可以直接写 DSL 运算、链式引用其他常量：

```rust
# use batch_impl::batch_trait;
# use std::rc::Rc;
trait TraitA {}
trait TraitB {}
batch_trait!(
    @nums=[u8, u16, u32];
    @uints=@uint;                      // 引用内置常量
    @wrapped=[Box, Rc]^@nums;          // 值含 DSL 运算（引用处求值）
    @chain=@wrapped;                   // 链式引用用户常量
    TraitA: @chain;
    TraitB: [Box, Rc]^@uints;
);
```

**引用可见性**：常量定义内只能引用**内置常量或此前已定义**的用户常量——
循环引用（`@a=@a`）与前向引用（`@a=@b` 且 `@b` 定义在后）在定义处报错。

未知 `@xxx`、范围端点非法、自定义与内置重名、循环/前向引用均报
`compile_error!`。

## 12. 三个入口

| 宏                   | 用途                                                     |
|----------------------|----------------------------------------------------------|
| `#[batch_impl]`      | 属性宏，在 trait 定义上标注，宏参数即 DSL                |
| `#[batch_impl_only]` | 同上，但丢弃 trait 定义，只输出 impl 块                  |
| `batch_trait!`       | 函数式宏，对已声明的 trait 批量生成 impl（支持多 trait） |

三者接受相同的 DSL 参数。

### `#[batch_impl_only]`

trait 已在别处定义、只需批量生成 impl 的场景。trait 定义仍要写出（只用来读取方法签名），输出不含 trait：

```rust
# use batch_impl::batch_impl_only;
# trait Greet { fn hello(&self) -> &str; } // 真实 trait 在别处定义
#[batch_impl_only(usize #hello{"hi"})]
trait Greet { fn hello(&self) -> &str; } // 此 dummy 定义被丢弃
// → impl Greet for usize { fn hello(&self) -> &str { "hi" } }
```

支持 `#path::to::Trait:` 路径前缀，为外部模块中定义的 trait 生成 impl
（路径末尾标识符必须与本地 dummy trait 名一致；`#[batch_impl]` 不支持此前缀）：

```rust
# use batch_impl::batch_impl_only;
# mod ext { pub mod traits { pub trait TraitName {} } }
# use ext::traits::TraitName;
#[batch_impl_only(#ext::traits::TraitName: usize, isize)]
trait TraitName { }
// → impl ext::traits::TraitName for usize {}
// → impl ext::traits::TraitName for isize {}
```

### `batch_trait!`

对已声明的 trait 批量生成 impl，`;` 分隔多个 trait 段。语法：
`[unsafe] Trait路径: impl-specs`，接受与 `#[batch_impl]` 完全相同的 DSL 语法
（`:` 右侧），额外支持多 trait 段、路径 trait（如 `foo::C`）、unsafe 段：

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

## 13. 错误提示

所有 DSL 语法错误通过 `compile_error!()` 输出中文提示并指向源码位置，永不 panic：

| 错误输入                 | 错误信息                                                             |
|--------------------------|----------------------------------------------------------------------|
| `batch_trait!(;)`        | `batch_trait! 中期望 trait 名称`                                     |
| `batch_trait!(A)`        | `batch_trait! 中期望 ':' 分隔 trait 名称和 impl-specs`               |
| `batch_trait!(A: B::)`   | `batch_trait! 中期望标识符作为 trait 名称`                           |
| 裸 `where` 缺代码块      | `batch-impl: \`where\` 谓词后缺少代码块 {...}`                       |
| where 谓词引用未声明形参 | `batch-impl: 继承的 where 谓词 ... 引用形参 ...，请声明或手写 where` |
