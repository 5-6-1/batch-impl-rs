# batch-impl 教程

**v0.10.0**（2026-09-14）——健壮性、诊断与文档发布：`@N..M` 在每个位置都是排他（本次唯一的语义统一）、`where` 谓词在定型后校验、splat 在每个参数位置列表里展开、若干误导性诊断修好，本教程新增配套参考手册（`docs/reference.md`）。

渐进式学习 DSL：从一行 impl 开始，到高级矩阵组合。示例均为可编译代码（发布版英语教程的代码块同时是 doctest），每一步的产物都是普通 Rust——宏生成的 impl 与手写逐 token 等价。**配套查阅文档是 `docs/zh-CN/reference.md`**（合法性矩阵、诊断目录、上限与保证）。

## 0. 三个系统 + 一个操作符

batch-impl 的一切能力由三根柱子（0.0→0.6 持续打磨）+ 一个操作符（0.7.0）构成：

| 部分           | 记号                                          | 作用                                                           |
|----------------|-----------------------------------------------|----------------------------------------------------------------|
| **apply 系统** | `.` / 空格 / `[]` / `()`                      | 类型矩阵：把左侧容器/修饰符应用到右侧类型，列表展开成多个 impl |
| **指令系统**   | `#name` / `#fill` / `#delegate` / `#blanket`  | 从 trait 定义抄签名、批量填 body、委托调用、覆盖式委托         |
| **常量系统**   | `@u*` / `@scalar` / `@u8..u128` / `@name=...` | 宏元层：命名并复用类型矩阵条目，纯词法替换——列出的族到处可用，而自定义 `@name=...` 段**仅 `batch_trait!` 支持**（§6.3） |
| **`*` 操作符** | `*[...]` / `*(...)`                           | 摊平：把容器/生成器展开拼入外层列表——0.7.0 新增，全位置生效    |

**预处理顺序**（固定的四阶段管道）：`@` 常量展开 → `<>` 尖括号配对 → `#` 指令展开 → `where` 处理。顺序决定了你能把什么写进什么：`@` 的结果可以包含 `<>`（配对后处理）、`#` 的参数可以引用 `@` 展开的列表、`where` 最后看到的是完整结构。

**按任务找章节**——任务优先索引：

| 我想…… | 看 |
|---|---|
| 给很多类型实现同一个方法 | §1、§3 |
| 用包装矩阵（空格、`.`、列表）覆盖一批类型 | §2 |
| 让多个类型共用一个 body | §3 |
| 把容器/生成器拼进列表（`*`） | §4 |
| 声明泛型、继承或添加 bound、写限定类型 | §5 |
| 寻址生成的参数（`@N` / `@g_i` / 区间） | §6 |
| 抄签名、委托、覆盖式委托、用自己的宏扩展 | §7 |
| 用 `where` 约束、用 `impl{...}` 写原型、批量化已有 `impl` | §8 |
| 生成元组与笛卡尔矩阵 | §9 |
| 用引用、指针、`unsafe`、属性、`!`、`self` | §10 |
| 在几个入口宏之间选 | §11 |
| 搞清某个错误是什么意思 | §12 |
| 读一个完整真实文件 | §13 |

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

## 2. 类型矩阵：空格（与 `.`）

**空格是主推的写法**：容器/修饰符与它接收的类型并排写——链式累加参数（左结合）。

> **空格到底是什么**：空格**不是 token**——它是 token 之间的**间隔**（proc-macro2 会剥掉空白，DSL 只看到相邻性）。因此空格应用的意思就是"这些 token 相邻，应用它们"（`Box u8` = `Box<u8>`），这与 Rust 自身读取类型语法的方式完全一致（`Box<u8>` 就是 `Box` 与 `<u8>` 相邻）。不需要任何显式运算符符号——**没有分隔符这件事本身就是运算符**。

| 写法                   | 展开                                  |
|------------------------|---------------------------------------|
| `Box u32`              | `Box<u32>`                            |
| `HashMap u32 String`   | `HashMap<u32, String>`（左结合累加）  |
| `fn(A,B) C`            | `fn(A,B)->C`（也可写 `fn(A,B) -> C`） |
| `&u8`                  | `&u8`（修饰符链式应用）               |
| `Tr u8`                | `impl Tr for u8`（裸 trait 名）       |
| `[Box, Vec] u32`       | `Box<u32>, Vec<u32>`（列表展开）      |
| `HashMap<u8> String`   | `HashMap<u8, String>`（预填泛型追加） |
| `Box [u8, u16]`        | `Box<u8>, Box<u16>`（列表分发）       |
| `[Box, Vec] [u8, u16]` | 笛卡尔积共 4 项                       |

**`.` 是同一运算的右结合形态**——只在需要**嵌套**而非累加时使用。空格累加会把参数并排（`Box Box u8` = `Box<Box, u8>`——对多数容器是笔误）；`.` 嵌套把类型组合（`Box.Box.u8` = `Box<Box<u8>>`）：

| 写法                     | 展开                                 |
|--------------------------|--------------------------------------|
| `Box.Box.u8`             | `Box<Box<u8>>`（右结合嵌套）         |
| `&Box u8`                | `&Box<u8>`（修饰符作用于嵌套类型）   |
| `[Box, Vec] T`           | `Box<T>, Vec<T>`                     |
| `Box [T1, T2]`           | `Box<T1>, Box<T2>`                   |
| `[HashMap<K>, Vec<K>] V` | `HashMap<K, V>, Vec<K, V>`           |

> **什么时候用哪个**：一个容器/修饰符 + 一个类型——并排写（`Box u8`、`&u8`、`HashMap<u8> String`）。当类型本身需要是组合类型（`Box<Box<u8>>`、`&Box<u8>`）时，用 `.` 连接组合——空格会把每一部分当作独立参数。

优先级从低到高：`;` < `,` < 空格 < `.`，`()` 分组在所有运算符之上。

**裸 trait 名**按 impl trait 应用：`Tr u8` = `impl Tr for u8`、`Tr<A> u8` = `impl Tr<A> for u8`。要**类型** `Tr<u8>` 直接写 `Tr<u8>`。一般情况下，不推荐使用裸Tr。

> **注意**：`Box.Vec u32` 是错误写法（会被解释为 `Box<Vec, u32>`），应写为 `Box.Vec.u32`。误写时 rustc 的 E0107 会把渲染后的 `Box<Vec, u32>` 打在报错里——误写自明。

> **操作数严格性**：`.`/`,` 两侧必须有操作数——`A.`、`.A`、`,A`、`A,,B` 均报 `compile_error!`；仅**尾随逗号**（`A,` / `[A, B,]`）允许，`();`/`[]` 等括号是真实 token 不算空操作数。`;` 作为 `batch_trait!` 段落边界保持宽松。

```rust
# use batch_impl::batch_impl;
# use std::collections::HashMap;
#[batch_impl(Box.Vec.u32, HashMap<u8> String)]
trait T {}
// → impl T for Box<Vec<u32>> {}   ← `.` 嵌套：Box 应用到 Vec<u32>
// → impl T for HashMap<u8, String> {}
```

## 3. 列表与 body

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

**分发传播**：`[A, B]` 列表是分发源——除了作为目标/操作数，嵌套位置也会传播：

```rust
# use batch_impl::batch_impl;
#[batch_impl((u8, [u16, u32, u64]))]
trait T {}
// → impl T for (u8, u16) {}
// → impl T for (u8, u32) {}
// → impl T for (u8, u64) {}

#[batch_impl(Vec<[u8, u16, u32]>)]
trait V {}
// → impl V for Vec<u8> {}
// → impl V for Vec<u16> {}
// → impl V for Vec<u32> {}
```

规则：元组/泛型实参中出现 `[A, B]` → 笛卡尔积分发（多数组全组合）；嵌套数组递归拆到底（`Vec<[[A,B], C]>` → `Vec<A>`/`Vec<B>`/`Vec<C>`）；`(X, [A,B]).N` 的组合含数组由外层分发递归覆盖。注意：具体生成器与 fresh 生成器组合可能 E0119 重叠（fresh 数量/结构相同）——rustc 兜底，用不同 fresh 数量的生成器可避免。

### 独立/共享 body 合并

列表项可有独立 body，与共享 body 合并：

```rust
# use batch_impl::batch_impl;
#[batch_impl(
    [usize { fn name(&self) -> &'static str { "usize" } },
     isize { fn name(&self) -> &'static str { "isize" } },
     f32  { fn name(&self) -> &'static str { "f32" } }]
    { fn zero() -> Self { Default::default() } }
)]
trait Zero {
    fn zero() -> Self;
    fn name(&self) -> &'static str;
}
// → 每个 impl：独立 fn name + 共享 fn zero——不同方法共存
// → impl Zero for isize { fn zero() -> Self { Default::default() } fn name() -> &'static str { "isize" } }
```

## 4. splat `*`——摊平操作符（0.7.0 主角）

splat 的直觉来自 Python 的 `*` 解包——`[a, *b]` 拼接列表、`f(*args)` 展开参数。batch-impl 的 `*` 是同样的**单层解包**：splat 把容器/生成器展开拼入外层列表，恰好展开一层。

| Python     | batch-impl                                      |
|------------|-------------------------------------------------|
| `[a, *b]`  | `[A, *[B, C]]`——把列表拼入外层列表              |
| `f(*args)` | `T *(A, B, C)`——把生成器展开到参数位            |
| 单层解包   | `*((a,b),)` = 一个 `(a,b)` impl（元组保持完整） |

**动机**：`*` 把嵌套生成器压缩进多参容器。与其手写 `T [A,B,C] [A,B,C] [A,B,C]`（27 组合的嵌套列表），一行得到同样 27 个 impl：

```rust
# use batch_impl::batch_impl;
struct T<A, B, C>(A, B, C);   // 三参容器
struct A; struct B; struct C;
#[batch_impl(T *(A, B, C).3)]  // splat 幂：把 (A,B,C).3 展开到三个参数位
trait Matrix27 {}
// → 27 个 impl：T<A,A,A> / T<A,A,B> / ... / T<C,C,C>（与 T [A,B,C] [A,B,C] [A,B,C] 相同）
```

`*` 前缀把容器/生成器**展开拼入**（扁平化）外层列表——它是"参数位置列表"的通用摊平标记，**全位置生效**。

### 4.1 列表 / 元组内拼入

```rust
# use batch_impl::batch_impl;
#[batch_impl([u8, *[u16, u32, u64]])]
trait SplatList {}
// → impl SplatList for u8 {}
// → impl SplatList for u16 {} / u32 / u64

#[batch_impl((u8, u16, u32) *(u64, usize, i8))]
trait SplatConcat {}
// → impl SplatConcat for (u8, u16, u32, u64, usize, i8) {}
```

### 4.2 左操作数：分配与追加

左 splat 按来源括号分语义——`*[A,B] T` **分配**（`*[A.T,B.T]`——集合，对标 `TyArray`）、`*(A,B) T` **追加**（`*(A,B,...,T)`——列表，对标 `TyTuple`）。`[]` 是**集合**、`()` 是**序列**——splat 只是保留来源括号的基础容器语义，**不是新规则**；`TySplat::Array`/`TySplat::Tuple` 镜像 `TyArray`/`TyTuple`：

```rust
# use batch_impl::batch_impl;
#[batch_impl(*[Vec, Box] u8)]
trait Dist {}
// → impl Dist for Vec<u8> {} / Box<u8>（分配：每个元素各自应用 u8）

# struct Pair<X, Y>(X, Y);
# struct A; struct B;
#[batch_impl(Pair *(A, B))]
trait Concat {}
// → impl Concat for Pair<A, B> {}（右 splat = 多实参）
```

### 4.3 泛型实参与 trait 路径

`Foo<*(a,b)>` = `Foo<a,b>`（多实参单 impl——与 `Foo<[a,b]>` 分发区分）；trait 路径同样：

```rust
# use batch_impl::batch_impl;
struct Pair<X, Y>(X, Y);
struct A; struct B;
#[batch_impl(Pair<*(A, B)>)]
trait G1 {}
// → impl G1 for Pair<A, B> {}（一个 impl，两个实参）

#[batch_impl(Conv<*(A, B)> Pair<A, B> #cv{unimplemented!()})]
trait Conv<T, U>: Sized { fn cv(_v: T, _o: U) -> Self; }
// → impl Conv<A, B> for Pair<A, B> { fn cv(_v: A, _o: B) -> Self { unimplemented!() } }
```

泛型实参内的 splat 幂把笛卡尔结果逐对分发为一个 impl：

```rust
# use batch_impl::batch_impl;
struct Frac<T, U>(T, U);
#[batch_impl(Frac<*(*@u*).2>)]
trait Pow {}
// → impl Pow for Frac<u8, u8> {} ... impl Pow for Frac<usize, usize> {}（36 个 impl）
```

### 4.4 容器规则

`(...)` / `[...]` 组内是孤立 splat 时解析为对应容器、splat 作为**一个元素**保持——`(*(a,b))` 是元组 `( *(a,b) )`、`[*(a,b)]` 是数组 `[ *(a,b) ]`。splat 元素全程保持整体（**splat 存续**）只在 codegen 展开——最终渲染结果是 `(a, b)` / `[a, b]`。`(a)` 保持透明组、`[a]` 是切片。

```rust
# use batch_impl::batch_impl;
#[batch_impl((*(u8, u16)))]
trait C {}
// → impl C for (u8, u16) {}（孤立 splat 组 = 元组，splat 元素展开）
```

### 4.5 generator 重包

`*().N`——生成器 splat——提升 fresh 声明并把元组摊平进容器：

```rust
# use batch_impl::batch_impl;
struct Pair2<A, B>(A, B);
#[batch_impl(Pair2<*().2>)]
trait GSplat {}
// → impl<P0, P1> GSplat for Pair2<P0, P1>（摊平成两个实参）
```

### 4.6 合法位置

splat 是**参数位置列表**：它会拼进泛型实参（`Foo<*(a,b)>`）、trait 应用实参（`Conv<*(A,B)> X`）、元组元素（`(a, *(b,c))`）、数组元素（`[*(a),*(b)]`）、callable 的参数表（`fn(*(u8, u16))`，`Fn(*(A,B)) -> C` 是同一张表）、`<>` 声明块（`<*(A,B)>` → `<A, B>`）、内联 bound（`<T: Tr<*(u8, u16)>>` → `<T: Tr<u8, u16>>`）、`dyn` bound 尾巴（`dyn Tr<*(u8, u16)>`）与 spec 列表（`[*(a,b)]`）。

唯一**不**展开的位置是 `where` 谓词——那里由 DSL 报出而不是泄漏出去（§8）：该子句到输出全程 token 级。

`<>` 声明块里的 fresh **生成器**是定向错误（那个块**就是** impl 的参数表，其 fresh 永不会被使用）——把生成器写在类型上。既非 splat 也非指针的裸 `*` 定向报错。

完整的位置矩阵见 `docs/zh-CN/reference.md` §2 与 §4。

两条规则：`T.*(A,B,...)` ≡ `T<A, B, ...>`（右 splat = 扁平参数追加）；左 splat 按来源——`*[A,B] T` = `*[A.T,B.T]`（分配律）、`*(A,B) T` = `*(A,B,...,T)`（追加）。嵌套幂等（`*(*[a,b])` = `[a,b]`）、空 splat 无操作（`[a, *()]` = `[a]`）；`*const`/`*mut` 指针不受影响（按后续 token 区分）。

## 5. 泛型 `<>`：从声明到可编程实参

### 5.1 声明

```rust
# use batch_impl::batch_impl;
#[batch_impl(<T: Clone> Box<T>)]
trait CloneBox {}
// → impl<T: Clone> CloneBox for Box<T> {}

#[batch_impl(<const N: usize> [u8; N])]
trait ArrayLen {}
// → impl<const N: usize> ArrayLen for [u8; N] {}
```

### 5.2 `A<>` — trait 泛型照抄

`A<>` 把 trait 定义的泛型（含 bound 与 where 谓词）原样复制为 impl 泛型：

```rust
# use batch_impl::batch_impl;
#[batch_impl(A<> Vec<u8>)]
trait A<T: Clone, const N: usize> {}
// → impl<T: Clone, const N: usize> A<T, N> for Vec<u8> {}
```

这个简写属于 **spec 头部**（trait 应用处）：正是它在声明那些形参。写在别处——
例如目标类型上——空 `<>` 就是 §6.5 的**同步标记**，会被填上本 spec 的实参
（`Swap2<>` → `Swap2<T>`），绝不会在 spec 中间吐出一个声明块。

### 5.3 实参：多实参、嵌套、绑定

```rust
# use batch_impl::batch_impl;
struct Map<K, V>(K, V);
struct A; struct B; struct C;
struct Wrap<X>(X);
#[batch_impl(Map<A, B>)]                 // 多实参
trait M1 {}
#[batch_impl(Map<Map<A, B>, C>)]         // 嵌套结构保留（TyGeneric 嵌套）
trait M2 {}
#[batch_impl(Conv<u8, Item = u8> Wrap<u8>)]  // 关联类型绑定（trait 路径）
trait Conv<T> { type Item; }
```

### 5.4 `<>` 内的操作（0.7.0 可编程化）

泛型实参位置可以写完整的 DSL 表达式——这是 0.7.0 的结构化落地：

```rust
# use batch_impl::batch_impl;
struct Wrap<X>(X);
struct Pair3<A, B>(A, B);
struct A2; struct B2;

#[batch_impl(Wrap<()2>)]               // generator：<P0,P1> Wrap<(P0,P1)>
trait GenTup {}
// → impl<P0,P1> GenTup for Wrap<(P0, P1)>（元组保持单个实参）

#[batch_impl(Pair3<*()2>)]             // generator splat：<P0,P1> Pair3<P0,P1>
trait GenSpl {}
// → impl<P0,P1> GenSpl for Pair3<P0, P1>（摊平成两个实参）

#[batch_impl(Wrap<@u*>)]                // 常量族：6 个 impl（u8..usize）
trait ConstArg {}

#[batch_impl(Wrap<[A2, B2]>)]           // 数组：2 个 impl（Wrap<A2>/Wrap<B2>）
trait ListArg {}
```

### 5.5 同名继承与 trait where 继承

trait 泛型参数与 spec 实参同名时，bound 自动继承——而继承是**位置式**的，因此改名的参数照样保留它的谓词：

```rust
# use batch_impl::batch_impl;
#[batch_impl(<T> Box<T> where Box<T>: Clone)]
trait B2 {}
// → impl<T> B2 for Box<T> where Box<T>: Clone {}
```

```rust
# use batch_impl::batch_impl;
// 位置式继承：trait 的 `T` 与 spec 的第一个实参配对
#[batch_impl(<X> Store<X> usize)]
trait Store<T>
where
    T: Clone,
{}
// → impl<X: Clone> Store<X> for usize
```

### 5.6 限定类型：`<T as Tr>::Assoc`

限定类型通过 `::` 取关联项，三种拼写都支持：限定自身（`<T as Tr>::Assoc`）、带泛型实参的限定路径（`Foo<T>::Assoc`）、turbofish（`Foo::<u8>::Assoc`，渲染为 `Foo<u8>::Assoc`）。它们能出现在任何类型能出现的位置——目标、泛型实参、bound 内，以及嵌套：

```rust
# use batch_impl::batch_impl;
trait Tr { type Assoc; }
struct S;
impl Tr for S { type Assoc = u8; }
impl Tr for u8 { type Assoc = u16; }
struct Holder<T>(T);
struct Wrap<T>(T);

// 作目标：`impl Q for Holder<<S as Tr>::Assoc>`（= `Holder<u8>`）
#[batch_impl(Holder<<S as Tr>::Assoc>)]
trait Q {}

// 嵌套限定作泛型实参：
// `impl Q2 for Vec<<<S as Tr>::Assoc as Tr>::Assoc>`（= `Vec<u16>`）
#[batch_impl(Vec<<<S as Tr>::Assoc as Tr>::Assoc>)]
trait Q2 {}

// bound 内：`<T: From<<S as Tr>::Assoc>> Q3<T> Wrap<T>`
#[batch_impl(<T: From<<S as Tr>::Assoc>> Q3<T> Wrap<T>)]
trait Q3<T> {}
```

`::` 尾巴是**普通 Rust 路径文本**：它绝不重新解析，因此尾巴里的 DSL 记号（`Holder<T>::Assoc<@0>`）会得到定向错误，而不是漏进生成的 impl。

### 5.7 bound 位置、全局路径与 fn 具名参数

另外三种 Rust 拼写同样被接受：

```rust
# use batch_impl::batch_impl;
// 关联类型绑定在 bound 位置上对**任何** trait 路径合法：`dyn`、`for<'a>` 与
// 内联 bound 都能带。普通类型的实参仍是普通类型列表
// （`Vec<Item = u8>` 仍报定向错误）。
#[batch_impl(Box<dyn Iterator<Item = u8>>)]
trait Q4 {}

// 开头的 `::` 让路径成为**全局路径**——可作 spec 起始、可嵌在实参列表里，
// 也可接 `::` 尾巴。
#[batch_impl(::std::vec::Vec<u8>)]
trait Q5 {}

// `fn(...)` **指针**类型可以给参数起名。名字原样保留、`:` 之后的类型照常解析，
// 因此具名参数内可用 DSL 算子（`fn(v: Box<u8>) -> u8`）。
#[batch_impl(fn(x: u8) -> u8)]
trait Q6 {}
```

`Fn(x: u8)` **不**被接受：rustc 本身就拒绝 `Trait(...)` 语法里的具名参数（"does not support named parameters"），DSL 因此照实报出这条规则，而不是漏出一个令人困惑的 `expected type` 错误。

**目标以 `::` 开头时必须显式写出元素边界。** `<...>`（跟在 ident 之后）与 `::` 都是**当前路径的续接**，所以并列写法会把 trait 头和绝对路径目标粘成一条路径——trait 落进**类型位置**（E0782）。空格与 `.` 是元素边界，而在 trait 头下两者等价，因此写 `.`：

```rust
# use batch_impl::batch_impl;
// `@trait<u8> . ::std::string::String` → impl TrE<u8> for ::std::string::String
#[batch_impl(@trait<u8> . ::std::string::String)]
trait TrE<T = usize> { fn tag(&self) -> u8 { 7 } }
```

```text
@trait<u8> ::std::string::String   →  impl TrE for TrE<u8>::std::string::String   （粘连，E0782）
Tr<u8> (::some_mod::SomeType)      →  impl Tr for Tr<u8, ::some_mod::SomeType>    （组是实参追加）
::std::vec::Vec<u8>                →  impl Tr for ::std::vec::Vec<u8>             （单元素 spec 整体是目标）
```

edition 2024 里 `::name` 指**外部 crate**；要指本 crate 根写 `crate::...`。规则本身见 `docs/zh-CN/reference.md` §1.2。

## 6. `@` 常量系统（宏元层）

`@` 是 DSL 预留的**库专属常量命名空间**——`#` 被指令机制占用，`@` 提供"命名并复用类型矩阵条目"的能力。它是纯**词法替换**（宏元层）：展开结果进入后续管道，不参与任何域内解析。

### 6.1 内置常量

**名字族**（闭集——语言定义的类型集合）：`@u*`、`@i*`、`@f*`、`@num`、`@scalar`。各自展开为成员列表：

| 常量 | 展开为 |
|---|---|
| `@u*` | `u8, u16, u32, u64, u128, usize` |
| `@i*` | `i8, i16, i32, i64, i128, isize` |
| `@f*` | `f32, f64` |
| `@num` | 全部 `@u*` + `@i*` + `@f*` 成员（14 个类型） |
| `@scalar` | 原语标量（数字族 + `bool` + `char`，共 16 个） |

```rust
# use batch_impl::batch_impl;
#[batch_impl(Box @u*)]  // Box 应用 @u* 的每个成员
trait BoxRc {}
// → impl BoxRc for Box<u8> {} / Box<u16> / ... / Box<usize>
```

**范围族**：`@u8..u128`、`@i8..i128`、`@f32..f64`（含端点）——一族内的连续段（`@u8..u128` → `u8, u16, u32, u64, u128`）。usize/isize 只进名字族不进范围族。

### 6.2 懒展开与引用

常量值存**原样 token**，引用处拼接并递归展开——值可以是 DSL 运算值（`@uints=@uint`），也可以链式引用（`@a=@b`）。定义处拦截循环/前向引用（防无限递归）；裸范围端点引用（`@a=@u8` 无 `..`）定义处报错。

### 6.3 自定义常量段（仅 `batch_trait!`）

前导 `@name=值;` 段定义复用的常量（值可含链式引用与 DSL 表达式）。**`#[batch_impl]` / `#[batch_impl_only]` 不支持自定义常量，应当利用apply消除重复**——0.7.2 误加的特性已在 0.8.0 回退；属性宏矩阵直接用 `.`/空格/`*` 书写：

```rust
# use batch_impl::batch_trait;
# trait A {} trait B<T> {}
batch_trait! {
    @uints = @u*;
    A: @uints;
    B: <T> B<T> Vec<T>;
}
```

> **限制**：`batch_trait!` **不支持 `#` 指令**（`#fill`/`#delegate`/`#blanket`/开放扩展）——指令需要 trait 定义作签名真相源，而 `batch_trait!` 是函数式宏、拿不到 trait 定义。需要指令时请改用 `#[batch_impl]` / `#[batch_impl_only]`。


### 6.4 宏元层完整化：寻址代数 + 值类别

`@` 的“位置引用”是一个**寻址代数**——不是并列记号：

| 记号         | 派生关系                               | 展开为                                                                                     |
|--------------|----------------------------------------|--------------------------------------------------------------------------------------------|
| `@g_i`       | **原语**——组 g、位 i（跨数组分发稳定） | 第 g 个生成器组的第 i 个 fresh（`@0_0` → 第一个生成器的第一个 fresh）                       |
| `@N`         | `@g_i` 在单 impl 内按文档序摊平的下标  | 第 N 个 fresh 泛型名（`where{@0: Clone}` 中 `@0` → `P0`）                                  |
| `@all_fresh` | 全部 fresh 泛型                        | 每个 fresh 一个谓词（≡ `@0..`）；**已废弃**，请写 `@0..`                                    |
| `@N..=M`     | 连续段                                 | fresh N..=M，逗号分隔（`@0..=1` → `P0, P1`）                                               |
| `@N..`       | 到最后一个 fresh 的**开放**段          | 从 N 到最后一个 fresh，逗号分隔（`@1..` → `P1, P2, ...`）；N 越界时**为空**（arity 1 的 impl 不产生此类谓词，不报错） |

fresh 的显示名按文档序编号为 `P0, P1, ...`（与 impl 已用 ident 冲突时按表格字母序后缀逃逸：`P0A`、`P0B`、…、`P0Z`、`P0AA`）——展开把名字拼到 `@` 所在位置（where 谓词主体、target 元组元素、泛型实参），范围变成多个名字，`where` 尾部逐 fresh 复制。

> **Power-user tier**：`@g_i` / `@all_fresh` / `@N..M` 是高级寻址记号——日常从 `@u*` / `@all_methods` / `@0` 起步，只有谓词必须指名某个特定 fresh 时才动用。自 0.7.2 起整个 DSL 语法面冻结（见 README），这些记号的语义不再变化。

> **引用里的 `_` 是"组/位"分隔符**，不是 Rust 的数字分隔符：`@1_0` 是组 1、位 0。想要扁平下标就写 `@1000`（不带分隔符）；`@1_000` 的读法见参考手册 §5 的边界材料。

```rust
# use batch_impl::batch_impl;
#[batch_impl(()2 where @0..=1: Clone)]   // 范围糖：@0..=1 = @0, @1
trait RangeSugar {}
// → impl<P0,P1> RangeSugar for (P0,P1) where P0: Clone, P1: Clone

#[batch_impl(()3 where @0..: Copy)]       // = @all_fresh（从 0 到最后一个 fresh）
trait AllFresh {}
// → impl<P0,P1,P2> AllFresh for (P0,P1,P2) where P0: Copy, P1: Copy, P2: Copy

#[batch_impl(()3 where @1..: Copy)]       // 开放范围：从下标 1 起
trait OpenRange {}
// → impl<P0,P1,P2> OpenRange for (P0,P1,P2) where P1: Copy, P2: Copy
// （arity 1 的 impl 不产生任何谓词——那里 `@1..` 为空）
```

`@all_fresh` 与 `@0..` 等价；**`@all_fresh` 已废弃**——`@N..` 家族是推荐
写法（`@0..` 覆盖整段、`@1..` 覆盖尾部）。既有 spec 继续工作；新代码请
写 `@0..`。

**范围在单个 `@N` 能出现的任何位置都可用**（0.9.2）：除了上面的 where
谓词，范围的尾部也可以是关联类型路径，逐 fresh 复制——目标位置的
范围对照本 spec 生成器产出的 fresh 列表重新展开：

```rust
# use batch_impl::batch_impl;
struct Wrap3<A, B, C>(A, B, C);
#[batch_impl(Wrap3<*()3> where @0..: Clone { fn m(&self) {} })]
trait RangeAngle { fn m(&self); }
// → impl<P0,P1,P2> RangeAngle for Wrap3<P0,P1,P2> where P0: Clone, P1: Clone, P2: Clone

trait HasOut { type Out; }
#[batch_impl(Wrap3<*()3> where @0..: HasOut, @0..::Out: Clone { fn m(&self) {} })]
trait RangeAssoc { fn m(&self); }
// → where P0: HasOut, P0::Out: Clone, P1: HasOut, P1::Out: Clone, P2: HasOut, P2::Out: Clone
```

范围索引的 fresh 列表来自本 spec 的生成器（`*().N` / `().N`）；spec 无
fresh 泛型时范围报 "out of range"。

**impl 泛型声明位置同样可用**：`<@0..>` 把范围覆盖的每个 fresh 声明为
impl 参数——生成器放在 trait 实参（`GenConv<*().2>`），声明与谓词引用
同一批 fresh：

```rust
# use batch_impl::batch_impl;
struct DeclTarget;
#[batch_impl(<@0..> GenConv<*()2> DeclTarget where @0..: Clone { fn m(&self) {} })]
trait GenConv<T, U> { fn m(&self); }
// → impl<P0,P1> GenConv<P0,P1> for DeclTarget where P0: Clone, P1: Clone
```

（空的 `<@0..>`——spec 无 fresh 生成器——不产生任何参数，如同空的
`@1..` 谓词。）

**组内范围 `@L_N..`**（0.9.2）在**单个生成器组内**切片——`@g_i` 的组内对应物，
跨数组分发稳定。一个 spec 里有多个生成器（`<*().2>` → 组 0、`<*().3>` → 组 1）
时，`@1_0..` 只约束组 1 的 fresh：

```rust
# use batch_impl::batch_impl;
struct MultiTarget;
#[batch_impl(
    <@0..> <@1..> PairGen<*()2, *()3> MultiTarget where @1_0..: Clone
    { fn m(&self) {} }
)]
trait PairGen<A, B, C, D, E> { fn m(&self); }
// → impl<P0,P1,P2,P3,P4> PairGen<P0,P1,P2,P3,P4> for MultiTarget
//     where P2: Clone, P3: Clone, P4: Clone   ← 仅组 1（P0,P1 无约束）
```

`@L_N..`（到组尾开放）、`@L_N..M` / `@L_N..=M`（闭合）都可用；未知组报错
如同 `@g_i`。

`@N` 在**值位置**同样解析——`:` 后的类型可在尖括号组内携带 `@N`，例如关联
类型绑定引用另一个 fresh 的关联类型（alga2 元组 `Module` 标量相等约束）：

```rust
# use batch_impl::batch_impl;
#[batch_impl(
    Module<(), ()> ()1..=4 where @0..: Module<(), (), Scalar: Copy>,
        @1..: Module<(), (), Scalar = @0::Scalar>
        impl{(A@..)} impl{@{}}
    #Scalar{@{0}::Scalar}
    #scale{( @(@A::scale(&self.@0, s),).. )}
)]
trait Module<Add, Mul> {
    type Scalar;
    fn scale(&self, s: Self::Scalar) -> Self;
}
// arity 2 → impl<P0,P1> Module<(), ()> for (P0,P1)
//   where P0: Module<(), (), Scalar: Copy>, P1: Module<(), (), Scalar: Copy>,
//         P1: Module<(), (), Scalar = P0::Scalar>
```

共享标量模式：从第二分量起每个分量声明 `Scalar = @0::Scalar`（第一分量的
标量），`@0` 解析为第一个 fresh 的名字。`@1..` 开放范围正好是"从第二分量
起"的集合——随元组 arity 收缩，arity 1 时消失。

另一根轴（值类别）：

| 记号              | 类别                                             | 用途                                          |
|-------------------|--------------------------------------------------|-----------------------------------------------|
| `@trait`          | **身份**——当前 trait 名/路径（batch_trait 段级） | 跨段打包「泛型声明 + trait 名」               |
| `@all_methods` 等 | **选择**——从 trait_def 提取 item 集合            | `#fill(@all_required_methods, -foo)` 精确选中 |
| `@Cow`            | **`#blanket` 专属内置包装常量**——`Cow<'_>` 及其固有约束（`@0: ToOwned + ?Sized, @0::Owned: @trait`） | blanket 可用的 `Cow` 委托（见 §7.4）          |

`@all` 系与 `-` 减法组合出任意 item 子集（`#fill(@all_required_methods, -foo)`）；`@all_default*` / `@all_required*` 区分默认实现与必需方法。

`X<>`（空尖括号）会同步为本 spec 的 trait 应用——写 `Semiring<>` 而不用重复
`Semiring<Additive, Multiplicative>`：

```rust
# use batch_impl::batch_impl;
# struct Additive;
# struct Multiplicative;
#[batch_impl(
    Semiring<Additive, Multiplicative> ().1..=2 where @0..: Semiring<> {},
)]
trait Semiring<Oa, Om> {}
// → impl<P0> Semiring<Additive, Multiplicative> for (P0,)
//     where P0: Semiring<Additive, Multiplicative>
// → …… arity 2（P1 同谓词）
```

它填充 **where 谓词**、`impl{...}` 模板、impl 泛型 bound 以及**目标类型**（含
`dyn … + X<>` 尾巴）——凡是标记出现在 impl 类型结构里的地方。`@trait<>` 等价
（`@trait` 先展开为 trait 路径）。该标记**不看名字**：填进去的是本 spec 的实参，
所以 `Other<>` 会变成 `Other<…spec 实参…>`（那里的元数不匹配由 rustc 报）；无泛型
参数的 trait 同步为裸名（`Tr<>` → `Tr`）。**body 内部**
通过**开关模板** `impl{Tr<>}` 同步——只含空括号 trait 的模板，不参与
Self 匹配，仅声明 body 里的 `Tr<>` 引用也同步（body 是任意 Rust，`Vec<>`
不是 trait 引用）。

### 6.5 bound 生成器：impl 泛型 bound 里的 Fn 族类型

生成器可以在 **impl 泛型 bound 内部**运行：`Fn()N`（以及 `FnMut` /
`FnOnce`）生成 Fn 的参数列表，其 fresh 参数**提升到 impl 泛型**
（`impl<P0,P1, T: Fn(P0,P1)>`——绝不会把泛型声明留在谓词内部，rustc 会
拒绝），target 引用同一批 fresh。这就是"按 Fn arity 一条 spec 生成多个
impl"的形式：`<R, T: Fn()0..4 R> Tr<T> (@0..)` 对 arity 0..4（排他）
各生成一个 impl，每个 impl 的 bound 固定为该 arity、target 元组按该 impl
自己的 fresh 重新展开：

```rust
# use batch_impl::batch_impl;
#[batch_impl(<R, T: Fn()0..3 R> MultiArity<T, R> (@0..) {
    fn arity(&self) -> usize { 0 }
})]
trait MultiArity<T, R> { fn arity(&self) -> usize; }
// → impl<R, T: Fn() -> R>          MultiArity<T, R> for ()
// → impl<R, P0, T: Fn(P0) -> R>    MultiArity<T, R> for (P0,)
// → impl<R, P0,P1, T: Fn(P0,P1)->R> MultiArity<T, R> for (P0,P1)
```

`Fn()N R`——空格 apply 返回类型——渲染为 `Fn(P0,..) -> R`（等价
`-> R`）。`FnMut` / `FnOnce` 渲染各自的 trait 名；裸 `fn.().N` 也可用
（作为**类型**——`fn` 不是 trait，不能作 bound，但同样的生成器形式出现
在类型位置）。target 里的 `@N..` 范围**每个 impl 独立展开**，所以每个
arity 的元组元素恰好是该 impl 的 Fn 参数（arity 0 份的空 `@0..` 坍缩为
`()`）。

target 元组的尾逗号**可选**：`(@0..)` ≡ `(@0..,)`——无逗号的括号内若只
有范围占位符，会按元组重开，arity 1 份仍然渲染真正的 1 元组 `(P0,)`
（绝不会是分组 `(P0)`）。

**多个 bound 生成器**在同一 spec 中按 arity 笛卡尔积分发，target 用
**组内范围**寻址每个生成器的 fresh（第一个 bound 的 fresh 用 `@0_0..`，
第二个用 `@1_0..`）——扁平 `@N..` 跨全部组索引，两个扁平范围会在同一
元组里重叠。组内范围要求其组存在（`Fn()0..N` bound 的 arity 0 份没有
该组 fresh，引用在那里报错——与 `@g_i` 同规则）。

## 7. 指令系统 `#`

指令从 trait 定义抄 item 签名（方法/const/type 全支持），body 由你填——"声明数据，而不是编写重复代码"。

### 7.1 `#name{body}` — 单 item 赋值

```rust
# use batch_impl::batch_impl;
#[batch_impl(usize #to_str{"usize"})]
trait ToString { fn to_str(&self) -> &str; }
// → impl ToString for usize { fn to_str(&self) -> &str { "usize" } }
```

### 7.2 `#fill(methods){body}` — 多方法同一 body

```rust
# use batch_impl::batch_impl;
#[batch_impl(u8 #fill([add, sub]){ todo!() })]
trait Arith { fn add(&mut self, x: u8); fn sub(&mut self, x: u8); }
```

参数可以是名字列表、`@all` 系 marker，配合 `-name` 排除：

```rust
# use batch_impl::batch_impl;
#[batch_impl(u8 #fill(@all_methods, -default_method){ 0 })]
trait Markers {}
```

> 只填一个方法时，`#fill([foo]){body}` 与单 item 指令 `#foo{body}` 等价，后者更简洁。

### 7.3 `#delegate(methods){target}` — 委托调用

```rust
# use batch_impl::batch_impl;
#[batch_impl(
    Vec<u32> #d_len{self.len()},
    Box.Vec.u32 #delegate(d_len){**self}
)]
trait MyLen { fn d_len(&self) -> usize; }
// → impl MyLen for Box<Vec<u32>> { fn d_len(&self) -> usize { (**self).d_len() } }
```

#### 改名委托目标：`foo = call_foo`（0.9.4）

元素 `foo = call_foo` 把 trait 的 `foo` 方法委托给目标的 `call_foo` 方法——delegate crate 的 `#[call(...)]` 机制，用 DSL 的 `=` 绑定拼写。签名保留 `foo`，只有调用用 `call_foo`：

```rust
# use batch_impl::batch_impl;
struct Wrapper(String);
impl Wrapper {
    fn len(&self) -> usize {
        self.0.len()
    }
}
#[batch_impl(Wrapper #delegate(size = len){self})]
trait HasSize { fn size(&self) -> usize; }
// → impl HasSize for Wrapper { fn size(&self) -> usize { (self).len() } }
```

绑定语义：每个被选方法绑定一个目标——默认同名，改名则绑定 `=` 右侧。改名的左侧**尚未选中**时把该方法加入选择集（`#delegate(size=len)` 单独选中 `size`）；与选中集重叠时**合并**（`#delegate(@all, size=len)`——`size` → `len`，其余同名，不产生重复定义）；同一方法改名两次报错。

### 7.4 `#blanket(methods){包装列表}` — 覆盖式委托

包装任意类型（含智能指针），逗号分隔，`:N` 标注 deref 深度（默认 1，`&`/`Box` 这类单层包装不用写）：

```rust
# use batch_impl::batch_impl;
#[batch_impl(#blanket(@all_methods){&, Box})]
trait Len { fn len(&self) -> usize; }
// → impl<T: Len> Len for &T { fn len(&self) -> usize { (*self).len() } }
// → impl<T: Len> Len for Box<T> { fn len(&self) -> usize { (**self).len() } }
```

> **`:N` deref 深度**——委托体要解引用多少层才能到达内部 `T`。单层包装（`&`、`Box`、`Rc`）默认 **1**，不用写：body 解引用 N+1 次（`&`/`Box` → `**self`）。`:2` 表示包装本身嵌套两层——`Box.Arc:2` = `Box<Arc<T>>`，委托体 `***self`。只有嵌套包装才写 `:N`；单层包装什么都不用写。

> **按值接收者**：`fn consume(self)` 的委托体是 `(*self).consume()`——按值 `self` 本身就是包装，少一层 deref（`&self` 方法才是 `(**self)`，穿透引用再穿包装）。移出语义对共享包装（`&`/`Rc`）不可过类型检查，生成物会带一条 `#[doc]` 提示（proc macro 无稳定 warning 通道，E0658）；跳过这类方法用 `@all_ref_methods`（保留 trait 默认），或手写 `#name{...}`。

#### GAT、`Self` 与非 Sized 目标（0.9.4）

**泛型关联类型（GAT）** 用带自身参数的投影委托——`trait Iterable { type Iter<'a> where Self: 'a; }` 变成 `type Iter<'a> = <T as Iterable>::Iter<'a> where Self: 'a;`（裸投影缺生命周期实参，E0107）。普通关联类型/常量保持既有 `<T as Trait>::Item` 投影。

**裸 `Self`** 出现在方法的参数或返回里无法 blanket 委托（转发产生内部类型，匹配不上包装的 `Self`）——定向报错并建议 `#name{...}`。`Self::Assoc` **返回**（`fn iter(&self) -> Self::Iter`）放行——内部 `T` 携带同一关联类型。

**`@?` 非 Sized 后缀**：以 `@?` 结尾的包装（`Box@?`）给该 spec 的 where 子句加 `T: ?Sized`，fresh 泛型可以是非 Sized 目标：

```rust
# use batch_impl::batch_impl;
#[batch_impl(#blanket(@all_methods){Box@?})]
trait DynLen { fn dlen(&self) -> usize; }
impl DynLen for str { fn dlen(&self) -> usize { self.len() } }
// → impl<T: DynLen + ?Sized> DynLen for Box<T> — T（以及目标）可以是非 Sized
```

#### `@Cow`——携带约束的打包（示范案例）

`@Cow` 是 **`#blanket` 专属内置包装常量**（只在 `#blanket` 包装列表里可用）。`Cow<'_>` 的 deref 目标是 `T::Owned` 而非 `T`——朴素 `(**self)` 委托过不了类型检查。`@Cow` 把 `Cow<'_>` **连同**固有约束谓词（`@0: ToOwned + ?Sized, @0::Owned: @trait`）打包，让 blanket 可用。这就是“常量只有携带约束才有复用价值”的示范：

```rust
# use batch_impl::batch_impl;
# use std::borrow::Cow;
#[batch_impl(#blanket(@all_methods){@Cow})]
trait CowLen { fn clen(&self) -> usize; }
impl CowLen for str { fn clen(&self) -> usize { self.len() } }
impl CowLen for String { fn clen(&self) -> usize { self.len() } }
// → impl CowLen for Cow<'_, str> ... / Cow<'_, String> ...（经由打包的谓词委托）
```

### 7.5 开放扩展（顶层宏注入）

未知指令 `#name(args){body}` 成为顶层宏调用——`{! m!{(arg1){arg2} trait_def}}` 形式把宏调用提升到顶层输出（示例用 crate 自带的**参考实现宏** `batch_preprocess_test`；宏参数里的 `trait_def` 提供签名，外部需已有同名 trait）。**扩展点的交付物是协议形状本身**——batch-impl 不实现你的 codegen，只保证 `{spec}(args){body}trait_def` 四段输入到达你的同名宏：

```rust,ignore
# use batch_impl::batch_impl;
# use batch_impl::batch_preprocess_test;
#[batch_impl(u16 {! batch_preprocess_test!{(add,inc){*self += 3} trait AddIncU16 { fn add(&mut self, x: u16); fn inc(&mut self); }}})]
trait AddIncU16 { fn add(&mut self, x: u16); fn inc(&mut self); }
```

> **协议已收敛为单一形态**：旧的**内嵌形态** `T {m!{...}}`（无 `!`，宏调用留在 impl body、输出关联项）自 0.7.2 起标注**弃用**（保留兼容，proc macro 无 warning 通道故为文档层面收敛）——新扩展一律按顶层 `{! m!{...}}` 四段协议 `{spec}(args){body} trait` 编写。

## 8. where 子句

### 8.1 `where` 谓词

`where` 子句给 impl 挂谓词。推荐**裸写**——`where 谓词 { 代码块 }`（谓词后
直接跟代码块）；`where{...}` 后缀（谓词在花括号里）等价且仍可用，只是多
一层 `{}`：

```rust
# use batch_impl::batch_impl;
#[batch_impl(Vec<u8> where Vec<u8>: Clone)]
trait T {}
```

### 8.2 裸写 `where 谓词 {代码块}`

约束与代码块分离的 Rust 风格写法（谓词后的 `{...}` 代码块必须存在）：

> 等价地，`where{谓词} {代码块}`（§8.1 后缀 + 链式 body）也可以裸写成 `where 谓词 {代码块}`，省一层 `{}`。

```rust
# use batch_impl::batch_impl;
#[batch_impl(u8 where u8: Clone { fn tag(&self) -> &'static str { "u8" } })]
trait T { fn tag(&self) -> &'static str; }
```

### 8.3 谓词继承与 `@N` 引用

trait 级 where 谓词按**位置替换**并入 impl：改名的参数照样保留它的谓词，谓词文本跟随同一位置上的实参（§5.5、参考手册 §7.2）。谓词指到 impl 没声明的东西时逐字通过，因此由 rustc 报未知类型。`@N` 在谓词中引用 fresh 名（`where{@0: Clone}`）；`@N..=M` 批量引用范围。裸 splat 作谓词主体明确报错（`where{*(A,B): Trait}` 无定义语义）；包进元组也没用（谓词内 splat 不展开）——分开写谓词。

### 8.4 `impl{...}` shape template 形状模板（0.8.0）

**一句话：模式匹配 + 文本替换。** 你写一个 `impl{...}` 块放**原型类型**，
宏把它与每个叶子目标类型**逐位匹配**——**相同**的 ident 原样保留，
**不同**的变成命名槽，槽名随后被**替换进** body（与 where 谓词）。一个 body 适配所有叶子：

```rust
# use batch_impl::batch_impl;
# use std::rc::Rc;
#[batch_impl([Box, Rc] u32 impl{W<T>} { fn mk(x: u32) -> W<T> { W::new(x) } })]
trait Make { fn mk(x: u32) -> Self; }
// → impl Make for Box<u32> { fn mk(x: u32) -> Box<u32> { Box::new(x) } }
// → impl Make for Rc<u32>  { fn mk(x: u32) -> Rc<u32>  { Rc::new(x) } }
```

匹配怎么工作，用大白话讲：

- **模板是作用在叶子类型上的模式**，逐位比较：`impl{W<T>}` 对叶子 `Box<u32>`——
  `W` ≠ `Box` 所以 `W` 是槽（`W := Box`）、`T` ≠ `u32` 所以 `T` 是槽
  （`T := u32`）；对 `Rc<u32>` 则 `W := Rc`、`T := u32`。模板本身**不是
  impl 目标**——它只声明槽。
- **槽被替换**——body（和 where 谓词）里出现的每个 `W`/`T` 都被替换成
  绑定的叶子部分。机制就这么多：匹配叶子、收集槽、替换。
- 裸 `impl{T}` 绑定**整个叶子**（`impl{T}` + `i32` → `T := i32`）；
  `impl{Rc<T>}` + `Rc<i32>` → 只有 `T := i32`（`Rc` 匹配、保留）。
- 多个 `impl{...}` 合并为单一映射——同形冗余绑定合法、异形冲突报错。
- 模板内 `@trait` 在匹配前展开为 trait path。

模板块内是**标准 Rust 类型**——DSL 算子被拒绝；`_` 是**通配**，匹配任意
东西且保持 `_`（详见下方模板匹配表）。

#### 模板匹配：哪些能绑定、哪些不能

模板与叶子按**结构递归**匹配——每种 `syn::Type` 形态都被识别并递归：

| 模板形态                                     | 行为                                                                    |
|----------------------------------------------|-------------------------------------------------------------------------|
| `T`（裸 ident）                              | 绑定整个叶子子树                                                        |
| `Rc<T>` / `std::rc::Rc<T>`（路径，多段也可） | base/段 ident：相同→字面、不同→槽；泛型实参递归                         |
| `&A` / `&mut A` / `*const A` / `*mut A`      | 引用/指针的生命周期与可变性只做结构比较；元素绑定                       |
| `[A]`（切片）、`(A, B, C)`（元组）           | 逐位绑定元素                                                            |
| `[A; 3]`（定长数组，字面长度）               | 长度逐字比较；元素绑定                                                  |
| `[A; N]`（定长数组，const 参数长度）         | 长度**绑定**叶子长度（`N := 3`；body 可用 `N`）                         |
| `[A; ()]`                                    | **保留形状**——变长段的内部标记（数组长度为 `()`，不可能出现在可编译代码中），不要在模板里手写 |
| `Cow<'_, A>`（生命周期实参）                 | `'_'` 是**通配**，匹配任意生命周期；`'a` vs `'b` 逐字；类型实参照常绑定 |

不能绑定（保持逐字比较——定向诊断而非静默误绑）：

- **fn 指针 / trait 对象模板内部**（`fn(A) -> B`、`dyn A + Send`）：整体逐字比较，只有完全相同的模板能匹配自身；
- **跨类实参绑定**（`Cow<'_, A>` 拆 1 实参的 `Box<u8>` 叶子；`Foo<A>` 拆 `Foo<3>`）：生命周期/const 实参不能绑定类型实参，arity 错位也无法对齐。改为每个形状族一个原型模板（下节）。

#### 原型实现模式

为**代表叶子**写一个正确实现，"相同→保留、不同→绑定"规则会自动适配矩阵中的每个叶子：

```rust
# use batch_impl::batch_impl;
# use std::rc::Rc;
#[batch_impl([Box, Rc] @num impl{Box<u8>} #max{Box::new(u8::MAX)})]
trait TMax { fn max() -> Self; }
// → impl TMax for Box<u8>  { fn max() -> Box<u8>  { Box::new(u8::MAX) } }
// → impl TMax for Box<u16> { fn max() -> Box<u16> { Box::new(u16::MAX) } }
// → impl TMax for Rc<f64>  { fn max() -> Rc<f64>  { Rc::new(f64::MAX) } }
```

每个形状族需要自己的原型（`Cow<'_, u8>` 模板覆盖 Cow 族——`'_'` 通配匹配任意叶子生命周期）。一族一族合并在同一条属性里，可写成独立 spec，也可写成成对 + 列表级分发：

```rust
# use batch_impl::batch_impl;
# use std::borrow::Cow;
# use std::rc::Rc;
#[batch_impl(
    [[Box, Rc] impl{Box<u8>},
     Cow<'_> impl{Cow<'_, u8>}] @num #tag{1}
)]
trait Tag { fn tag() -> usize; }
// Box<u8>..Rc<f64> 由 Box<u8> 原型覆盖；Cow<'_, u8>..Cow<'_, f64> 由 Cow 原型覆盖
// ——一条属性、两个形状族
```

#### 变长段与 body 重复块

`impl{...}` 模板可以用 `ident@..` 声明**变长段**：它覆盖从自身位置起的所有剩余元组位置（写在固定元素后面的段从固定元素个数开始）。尾随段**不需要逗号**——`impl{(A@..)}` 与 `impl{(A@..,)}` 等价（0.9.2；尾随逗号自动补上，模板仍解析为元组）。段元素按叶子绝对位编址，但**没有衍生名字**——写 `@A..` 不会占用、也不会声明 `A1`、`A2` 这些名字。要给某个元素起名，就在段旁边写一个普通固定元素（`impl{(A0, @A..,)}` 经普通槽通道绑定 `A0 := ` 叶子[0]，body 里直接裸写 `A0` 引用）。同层多段均分剩余位置（`(A@.., B@..,)` 匹配 arity-4 叶子 → A 长 2、B 长 2）；无法均分报错。段可递归进嵌套元组（`((A@..,),(B@..,))`）；同一模板内段名前缀重复报错。

body 用 `@(...)..` 重复：**重复块**按所引用段的元素数逐轮输出模式（与 Rust 声明宏的 `$( ... )*` 同一语义：每轮把实际绑定的元素直接拼进输出）：

```rust
# use batch_impl::batch_impl;
#[batch_impl((u8, u16, u32) impl{(A@..)} { fn tail(&self) -> (u8, u16, u32) { (@(@A::from(self.@0)),..) } })]
trait ShapeTail { fn tail(&self) -> (u8, u16, u32); }
// body → (u8::from(self.0), u16::from(self.1), u32::from(self.2))
```

- 块内 `@ident` 是**元素引用**——第 i 轮把段的第 i 个绑定叶子元素**直接**拼进输出（展开与最终 impl 之间不存在任何中间拼写）；
- 段旁边显式写的固定元素（`impl{(A0, @A..,)}` 里的 `A0`）是普通槽位：body 里哪里要用这个具名元素，就裸写 `A0`；
- `@N` 是**索引游标**——数字 `N + i`；路径前缀自己写（段从叶子下标 1 起就写 `self.@1`）；
- 块重复 L 次，长度有三个来源：块内引用的段（`@ident`，全部等长）、**前置段声明**（`@A(self.@0,)..`——`@` 后直接写段名，适合纯游标块）、或纯游标且无前置声明时用模板的**唯一段**（多段模板的纯游标块因长度歧义报错）；
- 每个重复块末尾的 `,` 是分隔符，每轮输出——**并列块之间不要再写逗号**（每个块已自带元素分隔）；也可把分隔符写在 `)` 与 `..` 之间（`@(x),..`），这样只在轮**间**输出、最后一轮之后不输出；`{...}` 代码块内的逗号按普通 Rust 规则——上述 DSL 分隔符只作用于重复块，不作用于代码块 body；
- 嵌套块独立轮次（笛卡尔积语义）——输出是各层轮数的乘积，按 body 封顶 65536 个输出 token（超限报 `repeat-block expansion produces N tokens (limit 65536)`）；
- 块外 body 中出现 `@` 报错；段元素没有 `@{...}` 拼写——该形式只承载 **fresh 位置引用**：`@{0}` 是本 impl 的第一个 fresh 泛型（显示名 `P0`）。body 里的 `@{N}` 需要声明 body 槽——`impl{@{}}`，或 fresh 绑定开关 `impl{@0..}`（其轮次消费 `@{N}`）——"用必先声明"规则。`@{@N}` 是**逐轮**形态：游标 `@N` 变成 `N + round`，纯游标块每轮命名自己的 fresh——`(@(@{@N}::foo()),..)` 在三个 fresh 上展开为 `(P0::foo(), P1::foo(), P2::foo())`。

少了该声明时看到什么（四条都是实测）：

```text
body 里写 @{0}，完全没有模板
  → batch-impl: a `@{N}` fresh reference in the body requires the `impl{@{}}`
    body-slot switch (declare it on the spec, e.g. `impl{@{}}`); without it,
    `@` in a body starts a repeat block

有 `impl{@{}}`，但这个 impl 没有 fresh 可指
  → batch-impl: `@0` is out of range — this impl has 0 fresh generics
    (numbered from 0 in document order; user-written params are addressed by name)

有 `impl{@{}}` 但没有 fresh 绑定开关，却写了纯游标块
  → batch-impl: a repeat block needs a driving segment or a fresh-binding
    switch (`impl{@0..}`) to determine its length

完全没有模板，却写了纯游标块
  → 没有 DSL 诊断——块原样到达 rustc，由它报
    expected one of `.`, `;`, `?`, `}`, or an operator, found `,`
    （由 tests/ui/impl_shape_repeat_no_driver.rs 锁定）
```

纯游标块生成元素引用而不用写类型名——元组到元组的整形场景：

```rust
# use batch_impl::batch_impl;
#[batch_impl((u8, u16, u32) impl{(A@..)} { fn elems(&self) -> (u8, u16, u32) { (@(self.@0,)..) } })]
trait ShapeElems { fn elems(&self) -> (u8, u16, u32); }
// body → (self.0, self.1, self.2)
// （单段模板提供长度；`@A(self.@0,)..` 是显式写法，多段模板也可用）
```

alga2 风格端到端——一条 spec 覆盖所有元组 arity，`@0..`（≡ `@all_fresh`）给每个 fresh 泛型加约束：

```rust
# use batch_impl::batch_impl;
trait Magma { fn combine(&self, rhs: &Self) -> Self; }
impl Magma for u8 { fn combine(&self, rhs: &Self) -> Self { *self + *rhs } }
#[batch_impl(
    ()1..=2 where @0..: Magma impl{(A@..)}
    #combine{( @(@A::combine(&self.@0, &rhs.@0),).. )}
)]
trait TupleMagma { fn combine(&self, rhs: &Self) -> Self; }
// → impl<P0> TupleMagma for (P0,) where P0: Magma { ... }
// → impl<P0, P1> TupleMagma for (P0, P1) where P0: Magma, P1: Magma { ... }
```

### 8.5 impl entry（0.8.0，ItemImpl 入口）

**同一个思路，更大的模板：整个 impl 块成为模式。** 不再用独立的
`impl{...}` 附件——你把一个普通 `impl` 块交给 `#[batch_impl]`，其 for-Type
持有占位槽名（`impl Make for A<B>`），再加一个 `模板 : 矩阵` 源。每个矩阵
叶子与 for-Type（`A<B>`）匹配，槽（`A := Box, B := usize`）被替换进整个块——
for-Type、where 谓词与 body——每个叶子产出一个 impl。原始 impl（含槽）被 withhold：

```rust
# use batch_impl::batch_impl;
# use std::rc::Rc;
# trait Make { fn make() -> Self; }
#[batch_impl(A<B> : [Box, Rc] [usize, isize])]
impl Make for A<B> { fn make() -> A<B> { A::new(B::default()) } }
// → impl Make for Box<usize> { fn make() -> Box<usize> { Box::new(usize::default()) } }
// → ... × 4
```

一句话：**写一个带占位符的 impl，每个矩阵格子得到一个 impl——与 §8.4
相同的匹配与替换，只是作用于整个块而非仅 body。**

- attr 语法：shape 形态 `A<B> : [Box,Rc] [usize,isize]`（模板 `:` 矩阵）或直接形态
  `<T> Box<T>`（泛型声明 + for-type，N = 1）；`;` 分隔多个 spec（`W:u8; W:u16`），
  单 spec 为常见形态；
- `@trait`（→ impl 的 trait path）允许在泛型声明 bound 与 where 谓词中；自定义
  `@` 常量与 `#` 指令在本入口拒绝。spec 里的生成器会把 fresh 泛型提升到 impl 上，
  `@N..` where 选择器据此解析（没有生成器时 `@N` 无可指对象，报越界）；
- impl 自带的泛型 / where 子句 / `unsafe` 保留；裸 where 谓词区域也以深度 0 `;`
  或（ItemImpl 仅）流末尾终止。
- **空** spec 列表（`#[batch_impl]`、`#[batch_impl()]`、`#[batch_impl(;)]`）是
  **无操作**：属性只负责从该块**派生** impl，没有可派生内容时原块被原样发射，而不是被扣下。
- **属性堆叠是同一次派生的多个 stage。** 块上方的第二个（第三个……）`#[batch_impl]`
  不是又一份 spec 列表：rustc 先展开**最外层**属性并把其余属性交给它，本入口把它们重新发射到
  自己派生的 impl 上，编译器随后在**那些 impl 上**展开下一步——于是各步按**源码顺序**
  （自上而下）作用于**累积中的块**：前一步留在原地的槽位由后一步绑定，各步合成为笛卡尔积。
  **空** stage 是恒等元——用它可以把某一步关掉：

  ```rust
  # use batch_impl::batch_impl;
  # struct Pair<A, B>(A, B);
  # trait Tag { fn tag(&self) -> u32; }
  #[batch_impl(A : [u8, u16])]      // stage 1 绑定 `A`
  #[batch_impl(B : [u32, u64])]     // stage 2 绑定 stage 1 留下的 `B`
  impl Tag for Pair<A, B> { fn tag(&self) -> u32 { 0 } }
  // → impl Tag for Pair<u8,u32> / Pair<u8,u64> / Pair<u16,u32> / Pair<u16,u64>
  ```
- **栈里属性的落点。** 写在两步之间的普通属性属于它所在的**展开层级**：它被发射到该 stage
  派生出的 impl 上，后续 stage 再从这些 impl 继承它。`#[cfg]` 的作用域也正由此确定——某一层的
  `#[cfg]` 会裁掉**该层派生的 impl 以及它下面的所有 stage**（实测：中层 `#[cfg(any())]` 时，
  下层那个必然报错的 stage 根本没有运行）。某层下面的 `#[cfg(test)]` 即恒真形态。
- **顺序为什么是必需的，而不只是约定。** 一个 *shape family*——头部形状各不相同的容器形态
  （`Vec<T>`、`[T; 4]`、`Box<[T]>`、`&[T]`）——在 §8.4 的模式里每族都需要一个 prototype，
  因为单个模板无法匹配四种不同形状的头部。用两步就能直接表达：第 1 步引入**留着元素槽的
  形状**，第 2 步填这个槽；而第 2 步的替换会**钻进**第 1 步产出的 token 内部（`B` 落在
  四个不同位置，其中一个在引用之后）：

  ```rust
  # use batch_impl::batch_impl;
  # trait Elem { fn elem_bytes(&self) -> usize; }
  #[batch_impl(A : [Vec<B>, [B; 4], Box<[B]>, &'static [B]])]
  #[batch_impl(B : [u8, u64])]
  impl Elem for A { fn elem_bytes(&self) -> usize { std::mem::size_of::<B>() } }
  // → impl Elem for Vec<u8> / Vec<u64> / [u8; 4] / [u64; 4]
  //                 / Box<[u8]> / Box<[u64]> / &'static [u8] / &'static [u64]
  ```

  把两条属性对调就会坏掉：元素在块还没提到它时就被绑定，随后形状那一步又引入了一个
  **没有人再绑定的** `B`——实测是四条 `E0425: cannot find type `B``（每个形状叶子一条），
  而不是八个可用 impl。正是 stage 顺序让"先形状、后元素"这件事可表达，而这个顺序就是 rustc
  属性展开给出的顺序（最外层先）——由 `tests/features/impl_entry_chain.rs` 锁定。

## 9. 元组生成与矩阵

### 9.1 `(A,)N` 长度展开

`(A,)N` 生成 1 元到 N 元元组（`(A,)`、`(A,A)`、…）：

```rust
# use batch_impl::batch_impl;
#[batch_impl((u8,)3)]
trait TuplePow {}
// → impl TuplePow for (u8,) {}
// → impl TuplePow for (u8, u8) {}
// → impl TuplePow for (u8, u8, u8) {}
```

范围：`(A,)2..4` / `(A,)2..=4` 生成区间长度。空元组 `()N` 是**生成器**——生成 N 个 fresh 泛型参数（见 5.4：`T<()2>` = `<P0,P1>T<(P0,P1)>`）。

### 9.2 笛卡尔积

`[A, B] [C, D]` 全组合；`*(A,B)2` splat 幂产生笛卡尔组合列表：

```rust
# use batch_impl::batch_impl;
# use std::rc::Rc;
#[batch_impl([Box, Rc] [u8, u16])]
trait Matrix {}
// → impl Matrix for Box<u8> {} / Box<u16> / Rc<u8> / Rc<u16>（4 项）
```

矩阵可以进一步包进容器或组合进更复杂的 spec（`([u8, u16],)2` 等）。

## 10. 修饰符大全

完整的修饰符表（`&`/`&mut`、`*const`/`*mut`、`unsafe`、`#[...]`、`!`、`self`）在 `docs/zh-CN/reference.md` §3；本节只留三个**读法容易搞错**的。

`&`、`*const`、`*mut`、`unsafe`、`fn` 类型、属性全支持：

```rust
# use batch_impl::batch_impl;
#[batch_impl(&str, &mut [u8], *const u8, *mut u8)]
trait Ptrs {}

#[batch_impl(unsafe fn(u8) -> u8)]
trait FnT {}

#[batch_impl(#[repr(C)] u8)]
trait Attr {}
```

> **`unsafe` 有两种角色**——`unsafe fn(A) -> B` 是 *unsafe fn 类型*：impl 本身保持安全（`impl Tr for unsafe fn(A) -> B`）。要把 **impl** 标记为 unsafe，用 `.` 应用 `unsafe`：`unsafe.fn(A) -> B` = `unsafe impl Tr for fn(A) -> B`。如果你写 `unsafe fn(...)` 却期待一个 unsafe impl，那就是写错了形式。

**`self` 前缀**是恒等前缀——`self T` = `T`。在矩阵里作"裸类型占位"：`[Box, self] u8` 生成 `Box<u8>` 与裸 `u8` 两个 impl（表达"包装 + 目标本身"）：

```rust
# use batch_impl::batch_impl;
#[batch_impl([Box, self] u8 { fn tag(&self) -> &'static str { "x" } })]
trait WrapOrBare { fn tag(&self) -> &'static str; }
// → impl WrapOrBare for Box<u8> {} / impl WrapOrBare for u8 {}
```

**`!`（never）作 fn 返回类型**：`fn(A) -> !` 合法——`!` 块没有 apply 语义，尾随 `{...}` 归属 impl：

```rust
# use batch_impl::batch_impl;
#[batch_impl(fn(u8) -> ! { fn call(&self, _: u8) -> ! { unreachable!() } })]
trait NeverRet { fn call(&self, x: u8) -> !; }
// → impl NeverRet for fn(u8) -> ! { fn call(&self, _: u8) -> ! { unreachable!() } }
```

**数组/切片 builder**：`[u8; 3]` 定长、`[u8]` 切片：

```rust
# use batch_impl::batch_impl;
#[batch_impl([u8; 3], [u8], &[u8])]
trait Slices {}
```

**任意嵌套类型原生支持**：`HashMap<String, Vec<(u8, u16)>>`、`Result<Box<dyn Fn(u8) -> u16>, String>` 等任意组合直接书写、结构化解析——DSL 已覆盖近乎全类型，不再是"原样透传"。

## 11. 入口

六个入口共用同一套 spec 文法；各入口的完整参数语义在 rustdoc（`src/doc/`），规则（`# 路径::到::Trait:` 前缀、impl 入口继承什么、`batch_trait!` 没有 `#` 指令）在参考手册 §9。

| 入口 | 形态 | 说明 |
|---|---|---|
| `#[batch_impl]` | 属性宏，挂在 `trait` 定义上 | 重发 trait 定义 + 生成 impl |
| `#[batch_impl]` | 属性宏，挂在 `impl` 块上（**impl 入口**，0.8.0） | 从一个手写 impl × 形状模板批量实例化 |
| `#[batch_impl_only]` | 属性宏，挂在 `trait` 定义上 | 只生成 impl，trait 来自外部（改名前缀 `# path::To::Trait:`） |
| `batch_trait!` | 函数式宏 | 分段 + 自定义 `@name=值;` 常量段；**不支持** `#` 指令 |
| `batch_preprocess_test!` | 测试用 | 只跑预处理、不断言生成物 |
| `batch_preview!` | 诊断通道 | 把展开结果作为 `compile_error!` 文本打印（唯一稳定的终端通道） |

```rust
# use batch_impl::batch_impl_only;
# mod path { pub mod to { pub trait Conv<T> { fn conv() -> T; } } }
# struct Wrapper<T>(T);
#[batch_impl_only(# path::to::Conv: Conv<bool> Wrapper<bool> #conv{false})]
trait Conv<T> { fn conv() -> T; }
// → impl Conv<bool> for Wrapper<bool> { fn conv() -> bool { false } }（trait 不重发）
```

```rust
# use batch_impl::batch_trait;
# trait A<T> {} trait B<T> {}
batch_trait! {
    @uints = @u*;
    A: @uints;
    B: <T> B<T> Vec<T>;
}
```

> **限制**：`batch_trait!` **不支持 `#` 指令**（`#fill`/`#delegate`/`#blanket`/开放扩展）——指令需要 trait 定义作签名真相源，而 `batch_trait!` 是函数式宏、拿不到 trait 定义。需要指令时请改用 `#[batch_impl]` / `#[batch_impl_only]`。

## 12. 错误提示

batch-impl 的错误是**编译期诊断**，指向最接近根源的用户可见 token（宏生成物 fallback 宏调用行）——**一条错误、不级联**。完整目录（每一类 + 锁定其措辞的 fixture）在 `docs/zh-CN/reference.md` §10。最常撞的是这些：

- **操作数缺失**：`A.` / `.A` / `,A` —— `compile_error!` 明确报错
- **`@N`/`@g_i` 越界或悬空引用**：`@5` 超出 impl 生成的泛型数 / `@2_0` 组不存在——用户语言定向报错（fresh 泛型从 0 按文档序编号）；生成名就是用户可见的显示名（`P0`、`P1`……），引用在宏内被拦截——绝不落为 rustc E0412 裸错
- **splat 作 where 谓词主体**：明确拒绝（`A, B: Trait` 无定义语义）——包进元组也没用：where 子句到输出全程 token 级，任何展开器都看不到谓词里的 splat（`(*(A,B)): Trait`、`X: Trait<*(A,B)>`）
- **`where` 谓词不是合法 Rust 谓词**：在谓词定型后（`X<>` 填充、`@` 解析、shape 模板槽替换之后）报错——`where{ A B }`（漏 `:`）直接给出修法，而不是对整个属性报解析错误
- **range 空**（`@u16..u8`）：报"空范围无 impl 生成"
- **trait 泛型改名没问题——继承是位置式的**：谓词里的 trait 参数按**位置**跟随（`trait Store<T> where T: Clone` 配 `<X> Store<X> usize` → `impl<X: Clone> Store<X> for usize`）；0.9 之前那条"改名中断继承"的拒绝已不存在（参考手册 §7.2）
- **裸 `*`（非 splat 非指针）**：定向错误而非 rustc 原始指针困惑
- **具体类型实参遇 `=`/`:`**：bound 与 binding 只属 trait 路径（`Conv<Item = u32> X`）与 **bound 位置**（`T: Iterator<Item = u8>`，`dyn` / `for<'a>` 内同理）；其余位置定向报错（`Assoc<Item = u32>` 配 struct 报 "binding args are only valid on a trait path … or in a bound"）
- **`<>` 声明块里的关联类型 binding**：声明块声明的是**参数**，因此 `<Item = u8> Target` 报 "an associated-type binding belongs on the trait application — write `Trait<Item = u8> Target`"。可用的写法是 trait 应用那种（它的 binding 会被提升进 impl body——Rust 里 `impl Trait<Item = u8> for X` 是 `E0229`）
- **blanket 方法带/返回裸 `Self`**：`#blanket` 无法委托带裸 `Self` 参数或返回裸 `Self` 的方法（转发得到内部类型，匹配不上包装的 `Self`）——报错并建议 `#name{...}`。`Self::Assoc` **返回**（`fn iter(&self) -> Self::Iter`）合法——内部 `T` 携带同一关联类型
- **`<>` 声明块里的 fresh 生成器**：报出可用写法（把生成器写在类型上，如 `T.*().2` 把生成的参数拼进去，或 `T<()2>` 把它们保持为一个元组实参）——那个块**就是** impl 的参数表，其 fresh 会被声明却永不被使用
- **已退役的 `^` 幂**：`(u8, u16)^2` 与 `T^()^2` 有自己的消息——幂是 `.N` 后缀（`(u8, u16).2`、`T.*().2`）——spec 链、角度块、`dyn` 尾巴里都报，**bound 位置**也报（那里此前会被静默丢弃）

## 13. 实战：仓库里那三个示例

上面每一章只讲一个机制。`examples/` 是它们**组合**成完整文件的地方，而且 CI 会编译它们，所以不会漂移：

| 示例 | 是什么 | 展示什么 |
|---|---|---|
| `examples/quickstart.rs`（约 320 行） | 可运行的单文件导览——`cargo run --example quickstart` 每个示例打印一行 `…: OK`，末尾给汇总 | 每个机制一个示例（§1–§8） |
| `examples/simplify.rs`（约 170 行） | 一个小型"数据检视"库：**29 个 impl** 出自约 15 行 DSL（手写约 80 行） | 列表 + 共享 body、包装委托、元组生成、空格应用、关联类型 binding、`#name`/`#fill`/`#delegate`、指针、三个入口 |
| `examples/typeclass.rs`（约 120 行） | type-class 层级（`Num` → `UNum`/`INum`/`FNum`）外加泛型分数的 `From<bool>` | `batch_trait!` 里的 `@` 家族、splat 幂（36 个实例）、trait 实参替换进抄来的 body |

### 13.1 `simplify.rs`——一个 trait 覆盖十二种数值

```text
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
```

一个列表 + 一个 body → **12 个 impl**：列表展开成 impl（§3），`{…}` 是共享 body，每个签名从 trait 定义抄来（§7）。`Self::default()` 对每种数值都是 0，所以一个表达式覆盖全部十二种。

文件其余部分：四个包装各一行委托给内层值（`[&, Box, Rc, Arc].T`，§7.3）、元组生成 `().1..=4`、左结合空格应用（`fn(i32, u32) String`、`HashMap u8 u16`）、带 `#name{…}` 的关联类型 binding 填单个 const、`#fill(name, kind){"u8"}` 让两个方法共用一个 body、一个 `batch_trait!` 段，以及 `*const` / `*mut` 目标。

### 13.2 `typeclass.rs`——一个类层级与 36 个实例

```text
#[batch_impl_only(
    From<bool>
    Frac<*(*@u*).2>
    #from{
        Frac { positive: true, num: value.into(), denom: true.into() }
    }
)]
pub trait From<T>: Sized {
    fn from(value: T) -> Self;
}
```

这里三个机制交汇：trait 应用 `From<bool>` **钉住**了 trait 的参数，于是抄来的签名 `fn from(value: T)` 变成 `fn from(value: bool)`（§7.2）；splat 幂 `Frac<*(*@u*).2>` 把 `@u*` 列表喂进**两个**泛型位——6 × 6 = 36 个 impl（§4）；`#from{…}` 提供整族共用的那一个 body（§6.3）。

它上面的层级展示了这个模式的另一半：`Num` 由 `#[batch_impl]` 定义并填充，而各子类先声明、再由 `batch_trait!` 配 `@` 家族**一行一个类**地填充（§6.1）——正是 type-class 需要的形状。

### 13.3 接下来看哪里

- 机制：上面的 §1–§12，然后是 `docs/zh-CN/reference.md`（合法性、诊断、上限）；
- 原始 API：`src/doc/*.md`（每个入口、每条指令一份）；
- 内部地图：`docs/zh-CN/architecture.md`。

