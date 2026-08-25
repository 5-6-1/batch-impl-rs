# batch-impl 教程

**v0.9.4**（2026-08-25）——**宏元层载体化重建 + blanket 委托与 `#delegate` 改名**：内部全部保留名替换为结构化载体（`@{g_i}` fresh 载体、`[前缀; ()]` 段标记、`@{A_pos}` 槽引用），fresh 显示名碰撞按字母序后缀逃逸（`P0A`、`P0B`、…）；用户可见特性：`#blanket` 委托 GAT（`type Iter<'a> = <T as Trait>::Iter<'a> where Self: 'a`）、裸 `Self` 参数/返回定向报错（`Self::Assoc` 返回放行）、wrapper `@?` 后缀加 `T: ?Sized`（`Box@?`——非 Sized 目标如 `Box<dyn Trait>`）；`#delegate(size = len)` 改名委托目标方法（见 §7.3）；生成的 impl 显示可读 fresh 泛型（`P0, P1, ...`）；`X<>` 在 `+` 连接 bound 内同步；fresh 范围在 impl body 内重开；repeat 块新增轮间分隔符与 fresh 驱动 cursor-only 块（`impl{@0..}` + `@@N` 名称引用，见 §8.4）。`@all_fresh` 已废弃（请写 `@0..`）。

渐进式学习 DSL：从一行 impl 开始，到高级矩阵组合。示例均为可编译代码（发布版英语教程的代码块同时是 doctest），每一步的产物都是普通 Rust——宏生成的 impl 与手写逐 token 等价。

## 0. 三个系统 + 一个操作符

batch-impl 的一切能力由三根柱子（0.0→0.6 持续打磨）+ 一个操作符（0.7.0）构成：

| 部分           | 记号                                          | 作用                                                           |
|----------------|-----------------------------------------------|----------------------------------------------------------------|
| **apply 系统** | `.` / 空格 / `[]` / `()`                      | 类型矩阵：把左侧容器/修饰符应用到右侧类型，列表展开成多个 impl |
| **指令系统**   | `#name` / `#fill` / `#delegate` / `#blanket`  | 从 trait 定义抄签名、批量填 body、委托调用、覆盖式委托         |
| **常量系统**   | `@u*` / `@scalar` / `@u8..u128` / `@name=...` | 宏元层：命名并复用类型矩阵条目，纯词法替换                     |
| **`*` 操作符** | `*[...]` / `*(...)`                           | 摊平：把容器/生成器展开拼入外层列表——0.7.0 新增，全位置生效    |

**预处理顺序**（固定的四阶段管道）：`@` 常量展开 → `<>` 尖括号配对 → `#` 指令展开 → `where` 处理。顺序决定了你能把什么写进什么：`@` 的结果可以包含 `<>`（配对后处理）、`#` 的参数可以引用 `@` 展开的列表、`where` 最后看到的是完整结构。

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
// → impl Zero for isize { fn zero() -> Self { 0 } fn name() -> &'static str { "isize" } }
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
#[batch_impl(Frac<*(*@u*)2>)]
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

`*()N`——生成器 splat——提升 fresh 声明并把元组摊平进容器：

```rust
# use batch_impl::batch_impl;
struct Pair2<A, B>(A, B);
#[batch_impl(Pair2<*()2>)]
trait GSplat {}
// → impl<P0, P1> GSplat for Pair2<P0, P1>（摊平成两个实参）
```

### 4.6 合法位置与限制

splat 是"参数位置列表"——凡是要元素列表的地方都展开：泛型实参（`Foo<*(a,b)>`）、元组/数组元素（`(a, *(b,c))`、`[*(a),*(b)]`）、泛型声明、fn 参数（`fn(*(A,B))`）、spec 列表（`[*(a,b)]`）。裸 splat 作 **where 谓词主体**没有定义语义（`*(A,B): Trait` 会展开成 `A, B: Trait`）——明确报错——包进元组（`(*(A,B)): Trait`）或分开写谓词；谓词**内部**的 splat（`X: Trait<*(A,B)>`）合法。

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

trait 泛型参数与 spec 实参同名时，bound 自动继承；改名则明确报错：

```rust
# use batch_impl::batch_impl;
#[batch_impl(<T> Box<T> where Box<T>: Clone)]
trait B2 {}
// → impl<T> B2 for Box<T> where Box<T>: Clone {}

#[batch_impl(<T> Foo<U>)]  // 改名（U ≠ T）→ 明确报错（不是静默）
trait Foo<T> {}
```

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
        impl{(A@..)}
    #Scalar{@{A_0}::Scalar}
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

where 谓词或 `impl{...}` 模板里的 `X<>`（**同名** trait 的空尖括号）会同步为
本 spec 的 trait 应用——写 `Semiring<>` 而不用重复
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

`@trait<>` 等价（`@trait` 先展开为 trait 路径）。任何非本 spec trait 的
`X<>` 报错；无泛型参数的 trait 同步为裸名（`Tr<>` → `Tr`）。**body 内部**
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

trait 级 where 谓词自动并入 impl；`@N` 在谓词中引用 fresh 名（`where{@0: Clone}`）；`@N..=M` 批量引用范围。裸 splat 作谓词主体明确报错（`where{*(A,B): Trait}` 无定义语义），包进元组或分开写。

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

`impl{...}` 模板可以用 `ident@..` 声明**变长段**：它覆盖从自身位置起的所有剩余元组位置（写在固定元素后面的段从固定元素个数开始）。尾随段**不需要逗号**——`impl{(A@..)}` 与 `impl{(A@..,)}` 等价（0.9.2；尾随逗号自动补上，模板仍解析为元组）。段元素按**叶子位置**编址——`(u8, A@..,)` 匹配 `(u8, u16, u32)` 时 A 覆盖位置 1、2（没有位置 0；索引游标从 `@1` 写起），body 里用**槽载体** `@{A_1}`、`@{A_2}` 引用（拼写是普通标识符 `A_1`：前缀 + `_` + 叶子绝对位）。同层多段均分剩余位置（`(A@.., B@..,)` 匹配 arity-4 叶子 → A 长 2、B 长 2）；无法均分报错。段可递归进嵌套元组（`((A@..,),(B@..,))`）；同一模板内段名前缀重复报错。

body 用 `@(...)..` 重复：**重复块**按所引用段的元素数逐轮输出模式：

```rust
# use batch_impl::batch_impl;
#[batch_impl((u8, u16, u32) impl{(A@..)} { fn tail(&self) -> (u8, u16, u32) { (@(@A::from(self.@0),)..) } })]
trait ShapeTail { fn tail(&self) -> (u8, u16, u32); }
// body → (@{A_0}::from(self.0), @{A_1}::from(self.1), @{A_2}::from(self.2))
//     → (u8::from(self.0), u16::from(self.1), u32::from(self.2))
//        → (u8::from(self.0), u16::from(self.1), u32::from(self.2))
```

- 块内 `@ident` 是**名字引用**——逐轮发射槽载体 `@{A_pos}`（pos = 叶子绝对位，如 `A_0`、`A_1`、…），随后由槽映射改写成绑定的叶子元素；
- `@N` 是**索引游标**——数字 `N + i`；路径前缀自己写（段从叶子下标 1 起就写 `self.@1`）；
- 块重复 L 次，长度有三个来源：块内引用的段（`@ident`，全部等长）、**前置段声明**（`@A(self.@0,)..`——`@` 后直接写段名，适合纯游标块）、或纯游标且无前置声明时用模板的**唯一段**（多段模板的纯游标块因长度歧义报错）；
- 块体末尾的 `,` 是分隔符，每轮输出——**并列块之间不要再写逗号**（每个块已自带元素分隔）；
- 嵌套块独立轮次（笛卡尔积语义）；
- 块外 body 中出现 `@` 报错。

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
  `@` 常量、`@N`/`@g_i` 引用与 `#` 指令在本入口拒绝；
- impl 自带的泛型 / where 子句 / `unsafe` 保留；裸 where 谓词区域也以深度 0 `;`
  或（ItemImpl 仅）流末尾终止。

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

## 11. 三个入口

| 入口                 | 语义                                                                                                        | trait 定义            |
|----------------------|-------------------------------------------------------------------------------------------------------------|-----------------------|
| `#[batch_impl]`      | 标准：impl + **重发 trait 定义**                                                                            | 标注在 trait 上       |
| `#[batch_impl_only]` | 只生成 impl，trait 由外部定义（可加 `# 路径::到::Trait:` 前缀声明外部 trait 的真实路径，要求至少一个 `::`） | 标注在 dummy trait 上 |
| `batch_trait!`       | 多段宏：多 trait + 段级 `@` 常量（不支持 `#` 指令）                                                         | 段内联                |

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

batch-impl 的错误是**编译期诊断**，指向最接近根源的用户可见 token（宏生成物 fallback 宏调用行）：

- **操作数缺失**：`A.` / `.A` / `,A` —— `compile_error!` 明确报错
- **未知 `@` 常量**：列出内置常量名（`@u*`/`@i*`/`@f*`/`@scalar`/`@num` + 范围族）
- **常量循环/前向引用**：定义处拦截（防无限递归）
- **`@N`/`@g_i` 越界或悬空引用**：`@5` 超出 impl 生成的泛型数 / `@2_0` 组不存在——用户语言定向报错，不泄露 `_Param_*_BatchGen_` 保留名（也不再以 rustc E0412 裸错呈现）
- **splat 作 where 谓词主体**：明确拒绝（`A, B: Trait` 无定义语义）
- **泛型改名不继承**：trait 泛型参数改名 = 明确报错，绝不静默
- **裸 `*`（非 splat 非指针）**：定向错误而非 rustc 原始指针困惑
- **range 空**（`@u16..u8`）：报"空范围无 impl 生成"
- **具体类型实参遇 `=`/`:`**：binding/bound 只属 trait 路径与泛型声明——定向报错（`Assoc<Item = u32>` 配 struct 报 "binding args are only valid on a trait path"）
- **类型位置的 `;`/`=`/`@`/`#`/`-` 残留**：定向报错（`..=` 的 `=` 除外，不级联二次诊断；孤 `-` 是已退役运算符——排除语义仅存于指令列表）
- **fn 参数列表后残留**：`fn(A) B` / `fn(A)->`——报意外 token（返回类型写 `-> B` 或 `fn(A) B`）
- **blanket 方法带/返回裸 `Self`**：`#blanket` 无法委托带裸 `Self` 参数或返回裸 `Self` 的方法（转发得到内部类型，匹配不上包装的 `Self`）——报错并建议 `#name{...}`。`Self::Assoc` **返回**（`fn iter(&self) -> Self::Iter`）合法——内部 `T` 携带同一关联类型
- **binding/bound 缺值**：`Conv<Item =>` / `Conv<T:> X`——报 "missing a value" / "missing a bound"
- **非整数类型字面量**：`1.5` / `"hi"` / `'a'`——类型位置只能是整数（usize）
- **range 端点非整数**：`1..x` / `A..B`——报 "needs integer endpoints"
- **数组长度畸形**：`[u8; 3; 4]` / `[u8;]`——报 "missing or malformed"
- **类型起始 `+`**：`+A`——报 "not valid at the start of a type"（`+` 属于 bound，如 `T: Clone + Send`）；前导 `.` 报操作数缺失；`?` 前缀类型（`?Sized`）透传由 rustc 报错
- **未知指令**：无内置拼写守卫——非内置指令、也非 trait item 名的 `#name(args){body}` 展开为你同名的宏（开放扩展）；拼写错误以 rustc 自己的 "macro not found" 呈现

