# batch-impl 教程

**v0.7.1**——0.7.0 在既有骨架上加入了 **`*` 摊平操作符**，并把 `<>`/`()`/`[]` 从"被动语法"升级为"可编程结构"：泛型实参内部现在可以写 generator（`()^N`）、splat（`*(A,B)`）、常量族（`@u*`）、列表（`[A,B]`）、绑定（`Item=u32`）与嵌套类型。

渐进式学习 DSL：从一行 impl 开始，到高级矩阵组合。示例均为可编译代码（发布版英语教程的代码块同时是 doctest），每一步的产物都是普通 Rust——宏生成的 impl 与手写逐 token 等价。

## 0. 三个系统 + 一个操作符

batch-impl 的一切能力由三根柱子（0.0→0.6 持续打磨）+ 一个操作符（0.7.0）构成：

| 部分 | 记号 | 作用 |
|---|---|---|
| **apply 系统** | `^` / `-` / `[]` / `()` | 类型矩阵：把左侧容器/修饰符应用到右侧类型，列表展开成多个 impl |
| **指令系统** | `#name` / `#fill` / `#delegate` / `#blanket` | 从 trait 定义抄签名、批量填 body、委托调用、覆盖式委托 |
| **常量系统** | `@u*` / `@scalar` / `@u8..u128` / `@name=...` | 宏元层：命名并复用类型矩阵条目，纯词法替换 |
| **`*` 操作符** | `*[...]` / `*(...)` | 摊平：把容器/生成器展开拼入外层列表——0.7.0 新增，全位置生效 |

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

## 2. 类型矩阵：`^` 与 `-`

`^` 与 `-` 是**同一运算**：左侧是修饰符/容器，右侧是目标类型。区别只在结合性：`^` 右结合（嵌套），`-` 左结合（累加参数）。

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

> **注意**：`Box^Vec-u32` 是错误写法（会被解释为 `Box<Vec, u32>`），应写为 `Box^Vec^u32`。误写时 rustc 的 E0107 会把渲染后的 `Box<Vec, u32>` 打在报错里——误写自明。

> **操作数严格性**：`^`/`-`/`,` 两侧必须有操作数——`A^`、`^A`、`-A`、`,A`、`A,,B` 均报 `compile_error!`；仅**尾随逗号**（`A,` / `[A, B,]`）允许，`();`/`[]` 等括号是真实 token 不算空操作数。`;` 作为 `batch_trait!` 段落边界保持宽松。

```rust
# use batch_impl::batch_impl;
# use std::collections::HashMap;
#[batch_impl(Box^Vec^u32, HashMap<u8>^String)]
trait T {}
// → impl T for Box<Vec<u32>> {}
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

规则：元组/泛型实参中出现 `[A, B]` → 笛卡尔积分发（多数组全组合）；嵌套数组递归拆到底（`Vec<[[A,B], C]>` → `Vec<A>`/`Vec<B>`/`Vec<C>`）；`(X, [A,B])^N` 的组合含数组由外层分发递归覆盖。注意：具体生成器与 fresh 生成器组合可能 E0119 重叠（fresh 数量/结构相同）——rustc 兜底，用不同 fresh 数量的生成器可避免。

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

| Python | batch-impl |
|---|---|
| `[a, *b]` | `[A, *[B, C]]`——把列表拼入外层列表 |
| `f(*args)` | `T-*(A, B, C)`——把生成器展开到参数位 |
| 单层解包 | `*((a,b),)` = 一个 `(a,b)` impl（元组保持完整） |

**动机**：`*` 把嵌套生成器压缩进多参容器。与其手写 `T-[A,B,C]-[A,B,C]-[A,B,C]`（27 组合的嵌套列表），一行得到同样 27 个 impl：

```rust
# use batch_impl::batch_impl;
struct T<A, B, C>(A, B, C);   // 三参容器
struct A; struct B; struct C;
#[batch_impl(T-*(A, B, C)^3)]  // splat 幂：把 (A,B,C)^3 展开到三个参数位
trait Matrix27 {}
// → 27 个 impl：T<A,A,A> / T<A,A,B> / ... / T<C,C,C>（与 T-[A,B,C]-[A,B,C]-[A,B,C] 相同）
```

`*` 前缀把容器/生成器**展开拼入**（扁平化）外层列表——它是"参数位置列表"的通用摊平标记，**全位置生效**。

### 4.1 列表 / 元组内拼入

```rust
# use batch_impl::batch_impl;
#[batch_impl([u8, *[u16, u32, u64]])]
trait SplatList {}
// → impl SplatList for u8 {}
// → impl SplatList for u16 {} / u32 / u64

#[batch_impl((u8, u16, u32)^*(u64, usize, i8))]
trait SplatConcat {}
// → impl SplatConcat for (u8, u16, u32, u64, usize, i8) {}
```

### 4.2 左操作数：分配与追加

左 splat 按来源括号分语义——`*[A,B]^T` **分配**（`*[A^T,B^T]`——集合，对标 `TyArray`）、`*(A,B)^T` **追加**（`*(A,B,...,T)`——列表，对标 `TyTuple`）。`[]` 是**集合**、`()` 是**序列**——splat 只是保留来源括号的基础容器语义，**不是新规则**；`TySplat::Array`/`TySplat::Tuple` 镜像 `TyArray`/`TyTuple`：

```rust
# use batch_impl::batch_impl;
#[batch_impl(*[Vec, Box]^u8)]
trait Dist {}
// → impl Dist for Vec<u8> {} / Box<u8>（分配：每个元素各自 ^u8）

# struct Pair<X, Y>(X, Y);
# struct A; struct B;
#[batch_impl(Pair^*(A, B))]
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

### 4.4 容器规则

`(...)` / `[...]` 组内是孤立 splat 时解析为对应容器、splat 作为**一个元素**保持——`(*(a,b))` 是元组 `( *(a,b) )`、`[*(a,b)]` 是数组 `[ *(a,b) ]`。splat 元素全程保持整体（**splat 存续**）只在 codegen 展开——最终渲染结果是 `(a, b)` / `[a, b]`。`(a)` 保持透明组、`[a]` 是切片。

```rust
# use batch_impl::batch_impl;
#[batch_impl((*(u8, u16)))]
trait C {}
// → impl C for (u8, u16) {}（孤立 splat 组 = 元组，splat 元素展开）
```

### 4.5 generator 重包

`T<*()^2>`——空 splat 的幂——生成 fresh 参数并摊平：

```rust
# use batch_impl::batch_impl;
struct Pair2<A, B>(A, B);
#[batch_impl(Pair2<*()^2>)]
trait GSplat {}
// → impl<_Param_0_BatchGen_, _Param_1_BatchGen_> GSplat for Pair2<..., ...> {}
//   （= <A,B> Pair2<A,B>——两个 fresh 实参）
```

### 4.6 合法位置与限制

splat 是"参数位置列表"——凡是要元素列表的地方都展开：泛型实参（`Foo<*(a,b)>`）、元组/数组元素（`(a, *(b,c))`、`[*(a),*(b)]`）、fn 参数（`fn(*(A,B))`）、spec 列表（`[*(a,b)]`）。裸 splat 作 **where 谓词主体**没有定义语义（`*(A,B): Trait` 会展开成 `A, B: Trait`）——明确报错——包进元组（`(*(A,B)): Trait`）或分开写谓词；谓词**内部**的 splat（`X: Trait<*(A,B)>`）合法。

两条规则：`T^*(A,B,...)` ≡ `T-A-B-...`（右 splat = 扁平参数追加——与 `-` 链等价，来源无关）；左 splat 按来源——`*[A,B]^T` = `*[A^T,B^T]`（分配律）、`*(A,B)^T` = `*(A,B,...,T)`（追加）。嵌套幂等（`*(*[a,b])` = `[a,b]`）、空 splat 无操作（`[a, *()]` = `[a]`）；`*const`/`*mut` 指针不受影响（按后续 token 区分）。

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

#[batch_impl(Wrap<()^2>)]               // generator：<P0,P1> Wrap<(P0,P1)>
trait GenTup {}
// → impl<P0,P1> GenTup for Wrap<(P0, P1)>（元组保持单个实参）

#[batch_impl(Pair3<*()^2>)]             // generator splat：<P0,P1> Pair3<P0,P1>
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
#[batch_impl(<T> Box<T> where{Box<T>: Clone})]
trait B2 {}
// → impl<T> B2 for Box<T> where Box<T>: Clone {}

#[batch_impl(<T> Foo<U>)]  // 改名（U ≠ T）→ 明确报错（不是静默）
trait Foo<T> {}
```

## 6. `@` 常量系统（宏元层）

`@` 是 DSL 预留的**库专属常量命名空间**——`#` 被指令机制占用，`@` 提供"命名并复用类型矩阵条目"的能力。它是纯**词法替换**（宏元层）：展开结果进入后续管道，不参与任何域内解析。

### 6.1 内置常量

**名字族**（闭集——语言定义的类型集合）：`@u*`、`@i*`、`@f*`、`@num`、`@scalar`。

```rust
# use batch_impl::batch_impl;
#[batch_impl(Box^@u*)]  // Box 应用 @u* 的每个成员
trait BoxRc {}
// → impl BoxRc for Box<u8> {} / Box<u16> / ... / Box<usize>
```

**范围族**：`@u8..u128`、`@i8..i128`、`@f32..f64`（含端点）。usize/isize 只进名字族不进范围族。

### 6.2 懒展开与引用

常量值存**原样 token**，引用处拼接并递归展开——值可以是 DSL 运算值（`@uints=@uint`），也可以链式引用（`@a=@b`）。定义处拦截循环/前向引用（防无限递归）；裸范围端点引用（`@a=@u8` 无 `..`）定义处报错。

### 6.3 batch_trait! 自定义常量段

`batch_trait!` 前导 `@name=值;` 段定义跨段复用的常量：

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

| 记号 | 派生关系 | 含义 |
|---|---|---|
| `@g_i` | **原语**——组 g、位 i（跨数组分发稳定） | 寻址宏生成的泛型 |
| `@N` | `@g_i` 在单 impl 内按文档序摊平的下标 | 引用 fresh 泛型名（`where{@0: Clone}`） |
| `@all_fresh` | 全部 fresh 泛型 | 范围糖——“每一个” |
| `@N..=M` | 连续段 | 范围糖——`@0..=1` = `@0, @1` |

```rust
# use batch_impl::batch_impl;
#[batch_impl(()^2 where{@0..=1: Clone})]   // 范围糖：@0..=1 = @0, @1
trait RangeSugar {}
// → impl<P0,P1> RangeSugar for (P0,P1) where P0: Clone, P1: Clone

#[batch_impl(()^3 where{@all_fresh: Copy})] // 全部 fresh 泛型
trait AllFresh {}
// → impl<P0,P1,P2> AllFresh for (P0,P1,P2) where P0: Copy, P1: Copy, P2: Copy
```

另一根轴（值类别）：

| 记号 | 类别 | 用途 |
|---|---|---|
| `@trait` | **身份**——当前 trait 名/路径（batch_trait 段级） | 跨段打包「泛型声明 + trait 名」 |
| `@all_methods` 等 | **选择**——从 trait_def 提取 item 集合 | `#fill(@all_required_methods, -foo)` 精确选中 |
| `@Cow` 等自定义 | **打包**——类型 + 固有约束一体 | 复用“带约束的包装”（见 §7.4） |

`@all` 系与 `-` 减法组合出任意 item 子集（`#fill(@all_required_methods, -foo)`）；`@all_default*` / `@all_required*` 区分默认实现与必需方法。

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

### 7.3 `#delegate(methods){target}` — 委托调用

```rust
# use batch_impl::batch_impl;
#[batch_impl(
    Vec<u32> #d_len{self.len()},
    Box^Vec^u32 #delegate(d_len){**self}
)]
trait MyLen { fn d_len(&self) -> usize; }
// → impl MyLen for Box<Vec<u32>> { fn d_len(&self) -> usize { (**self).d_len() } }
```

### 7.4 `#blanket(methods){包装列表}` — 覆盖式委托

包装任意类型（含智能指针），`:N` 标注 deref 深度：

```rust
# use batch_impl::batch_impl;
#[batch_impl(#blanket(@all_methods){[&, Box]})]
trait Len { fn len(&self) -> usize; }
// → impl<T: Len> Len for &T { fn len(&self) -> usize { (*self).len() } }
// → impl<T: Len> Len for Box<T> { fn len(&self) -> usize { (**self).len() } }
```

#### `@Cow`——携带约束的打包（示范案例）

`Cow<'_>` 的 deref 目标是 `T::Owned` 而非 `T`——朴素 `(**self)` 委托过不了类型检查。`@Cow` 把 `Cow<'_>` **连同**固有约束谓词（`@0: ToOwned + ?Sized, @0::Owned: @trait`）打包，让 blanket 可用。这就是“常量只有携带约束才有复用价值”的示范：

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

## 8. where 子句

### 8.1 `where{...}` 后缀

```rust
# use batch_impl::batch_impl;
#[batch_impl(Vec<u8> where{Vec<u8>: Clone})]
trait T {}
```

### 8.2 裸写 `where 谓词 {代码块}`

约束与代码块分离的 Rust 风格写法（谓词后的 `{...}` 代码块必须存在）：

```rust
# use batch_impl::batch_impl;
#[batch_impl(u8 where u8: Clone { fn tag(&self) -> &'static str { "u8" } })]
trait T { fn tag(&self) -> &'static str; }
```

### 8.3 谓词继承与 `@N` 引用

trait 级 where 谓词自动并入 impl；`@N` 在谓词中引用 fresh 名（`where{@0: Clone}`）；`@N..=M` 批量引用范围。裸 splat 作谓词主体明确报错（`where{*(A,B): Trait}` 无定义语义），包进元组或分开写。

## 9. 元组生成与矩阵

### 9.1 `(A,)^N` 长度展开

`(A,)^N` 生成 1 元到 N 元元组（`(A,)`、`(A,A)`、…）：

```rust
# use batch_impl::batch_impl;
#[batch_impl((u8,)^3)]
trait TuplePow {}
// → impl TuplePow for (u8,) {}
// → impl TuplePow for (u8, u8) {}
// → impl TuplePow for (u8, u8, u8) {}
```

范围：`(A,)^2..4` / `(A,)^2..=4` 生成区间长度。空元组 `()^N` 是**生成器**——生成 N 个 fresh 泛型参数（见 5.4：`T<()^2>` = `<P0,P1>T<(P0,P1)>`）。

### 9.2 笛卡尔积

`[A, B]^[C, D]` 全组合；`*(A,B)^2` splat 幂产生笛卡尔组合列表：

```rust
# use batch_impl::batch_impl;
# use std::rc::Rc;
#[batch_impl([Box, Rc]^[u8, u16])]
trait Matrix {}
// → impl Matrix for Box<u8> {} / Box<u16> / Rc<u8> / Rc<u16>（4 项）
```

矩阵可以进一步包进容器或组合进更复杂的 spec（`([u8, u16],)^2` 等）。

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

**数组/切片 builder**：`[u8; 3]` 定长、`[u8]` 切片：

```rust
# use batch_impl::batch_impl;
#[batch_impl([u8; 3], [u8], &[u8])]
trait Slices {}
```

**复杂类型透传**：`HashMap<String, Vec<(u8, u16)>>` 等任意组合原样传递。

## 11. 三个入口

| 入口 | 语义 | trait 定义 |
|---|---|---|
| `#[batch_impl]` | 标准：impl + **重发 trait 定义** | 标注在 trait 上 |
| `#[batch_impl_only]` | 只生成 impl，trait 由外部定义（可加 `# Path: ` 前缀） | 标注在 dummy trait 上 |
| `batch_trait!` | 多段宏：多 trait + 段级 `@` 常量 + `#` 指令 | 段内联 |

```rust
# use batch_impl::batch_impl_only;
# struct Wrapper<T>(T);
# trait Conv<T> { fn conv() -> T; }
#[batch_impl_only(Conv<bool> Wrapper<bool> #conv{false})]
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

- **操作数缺失**：`A^` / `^A` / `,A` —— `compile_error!` 明确报错
- **未知 `@` 常量**：列出内置常量名（`@u*`/`@i*`/`@f*`/`@scalar`/`@num` + 范围族）
- **常量循环/前向引用**：定义处拦截（防无限递归）
- **splat 作 where 谓词主体**：明确拒绝（`A, B: Trait` 无定义语义）
- **泛型改名不继承**：trait 泛型参数改名 = 明确报错，绝不静默
- **裸 `*`（非 splat 非指针）**：定向错误而非 rustc 原始指针困惑
- **range 空**（`@u16..u8`）：报"空范围无 impl 生成"
- **具体类型实参遇 `=`/`:`**：binding/bound 只属 trait 路径与泛型声明——定向报错（`Assoc<Item = u32>` 配 struct 报 "binding args are only valid on a trait path"）

