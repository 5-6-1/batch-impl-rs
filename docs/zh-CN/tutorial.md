# batch-impl 教程

**v0.7.0**——0.6.7 已发布；0.7.0：**splat** `*` 前缀（摊平容器/生成器到列表；左操作数 `*[...]` 分配 / `*(...)` 追加）、数组分发传播、`#fill` 单元素推荐（`#name{...}`）；0.6.x：receiver 过滤、`#blanket` 委托、span 诊断、泛型参数族、`@N` fresh 引用。
0.6.7：fresh 逐 impl 编号（`@N` 任意位置、含目标类型本身）、`@g_i` 分组引用、
顶层开放扩展（`{! ...}`——宏收到 `{spec}(args){body}trait` 并生成自己的 impl）、
`@all_fresh` / `@N..M` 批量 where 引用、错误聚合。

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

### splat（`*` 展开）

`*` 前缀把容器/生成器**展开拼入**（扁平化）——只出现在 `[]`/`()` 之前：

```rust
# use batch_impl::batch_impl;
#[batch_impl([u8, *[u16, u32, u64]])]
trait SplatList {}
// → impl SplatList for u8 {}
// → impl SplatList for u16 {} / u32 / u64

#[batch_impl((u8, u16, u32)^*(u64, usize, i8))]
trait SplatConcat {}
// → impl SplatConcat for (u8, u16, u32, u64, usize, i8) {}
//   （现状 `^` 是嵌套追加——`*` 给扁平拼接）

#[batch_impl((*(()^3)))]
trait SplatGen {}
// → impl<T0, T1, T2> SplatGen for (T0, T1, T2) {}   // 生成器 splat（组变元组）
```

语义：元组/数组内 `*X` → 元素拼入（`[a, *[d,e,f]]` = `[a,d,e,f]`）；`^`/`-` 右操作数 `*X` → 扁平追加（左元组拼接 / 左泛型多实参 `Vec^*(a,b)` = `Vec<a,b>`，与来源括号无关）；
左操作数按来源括号分语义——`*[A,B]^T` 分配（`*[A^T,B^T]`——集合，对标 `TyArray`）、`*(A,B)^T` 追加（`*(A,B,...,T)`——列表，对标 `TyTuple`）。泛型实参 `Foo<*(a,b)>` = `Foo<a,b>`（多实参单 impl——与 `Foo<[a,b]>` 分发区分）。

**统一容器规则**：组内是孤立 splat 时自动转为对应容器——`(*(a,b))` ≡ `(*(a,b),)` ≡ `(a,b)`（元组）、`[*(a,b)]` ≡ `[*(a,b),]` ≡ `[a,b]`（impl 列表/分发）；数组 splat 形式同理（`(*[a,b])` → `(a,b)`、`[*[a,b]]` → `[a,b]`）。一条代码路径、无特例：`(a)` 保持透明组、`[a]` 是切片。

**合法位置**：splat 是"参数位置列表"——凡是要元素列表的地方都展开：泛型实参（`Foo<*(a,b)>`）、元组/数组元素（`(a, *(b,c))`、`[*(a),*(b)]`）、泛型声明（`<*(A,B)>`）、fn 参数（`fn(*(A,B))`）、spec 列表（`[*(a,b)]`）。裸 splat 作 **where 谓词主体**没有定义语义（`*(A,B): Trait` 会展开成 `A, B: Trait`）——明确报错——包进元组（`(*(A,B)): Trait`）或分开写谓词；谓词**内部**的 splat（`X: Trait<*(A,B)>`）合法。

两条规则：`T^*(A,B,...)` ≡ `T-A-B-...`（右 splat = 扁平参数追加——与 `-` 链等价，来源无关）；左 splat 按来源——`*[A,B]^T` = `*[A^T,B^T]`（分配律——组合 `X^*[A,B]^T` = `X<A^T,B^T>` 一个 impl）、`*(A,B)^T` = `*(A,B,...,T)`（追加）。嵌套幂等（`*(*[a,b])` = `[a,b]`）、空
splat 无操作（`[a, *()]` = `[a]`）；`*const`/`*mut` 指针不受影响（按后续 token 区分）。

## 4. 泛型声明

```rust
# use batch_impl::batch_impl;
#[batch_impl(<T> Vec<T>)]
trait Collection {}
// → impl<T> Collection for Vec<T> {}
```

**约束写法规范**（0.6.1 起）：`<>` 只写名字，约束统一放 `where{...}`——

```rust
# use batch_impl::batch_impl;
#[batch_impl(<T> Named<T> Vec<T> where{T: Clone} { fn n(&self) -> usize { self.len() } })]
trait Named<T: Clone> { fn n(&self) -> usize; }
```

`<T: Clone>`（inline bound）仍兼容（未写约束时 trait 定义 bound 自动继承），
但**约束容器统一为 where** 后，多处约束的合并就是"并列谓词"（宏只做 token
拼接、零分析）——blanket 的 `T: Trait` 与包装谓词即因此天然合并。

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

> **单元素推荐 `#name{body}`**：只填一个方法/常量/类型时，写 `#name{body}`
> （如 `#N{5}`）而非 `#fill(name){body}`——更短且自文档化。`#fill` 用于
> **多个** item（`#fill(a, b)`、`#fill(@all_required_methods)`）。

> 指令参数可用 `(args)` 或 `[args]`——两者等价（如
> `#fill[@all_methods]{0}`）；方括号在嵌套/遮蔽场景（参数本身含括号）下更
> 清晰。`#name{body}`（无参数）两种写法通用。

```rust
# use batch_impl::batch_impl;
#[batch_impl(usize #fill(name, kind){"usize"})]
trait Describable { fn name(&self) -> &str; fn kind(&self) -> &str; }
// → 为 name 和 kind 各生成 { "usize" } body
```

特殊标记：`@all`（所有 item）、`@all_methods`（仅 fn）、`@all_constants`（仅 const）、`@all_types`（仅 type）。

**按默认实现状态过滤**（0.6.1 新增）：trait item 分"有默认实现"（fn 带默认体 /
const 带默认值 / type 带默认类型）与"无默认实现"（required，impl 必须提供）两种，
`@all_required*` / `@all_default*` 分别选取：

| 标记 | 选取范围 |
|---|---|
| `@all_required_methods` | 仅无默认实现的方法（impl 必须提供） |
| `@all_default_methods` | 仅有默认实现的方法（impl 可省略） |
| `@all_required` / `@all_default` | 对应状态的全部 item（fn + const + type） |
| `@all_required_constants` / `@all_default_constants` | 对应状态的 const |
| `@all_required_types` / `@all_default_types` | 对应状态的 type（**注意**：trait 关联类型的默认值 `type T = u8;` 是 nightly 特性（`associated_type_defaults`，stable 上 E0658）——`@all_default_types` 仅 nightly 场景可用；`@all_required_types` 的 `type T;` 声明 stable 可用） |

`@all_required_methods` 单独用 = "只实现必须的、默认方法保留 trait 默认实现"（比
`@all` + `-name` 逐个排除更精确）；`@all_default_methods` 需与 required 侧或手写
组合（只填默认方法会缺 required 实现 → E0046）。required ∪ default = all。
三指令（`#fill`/`#delegate`/`#blanket`）与 `-` 排除（`-@all_default_methods`）通用。

```rust
# use batch_impl::batch_impl;
// 必须的填 1，默认方法覆盖成 2
#[batch_impl(usize #fill(@all_required_methods){1} #fill(@all_default_methods){2})]
trait MixDefault {
    fn required(&self) -> u32;
    fn optional(&self) -> u32 { 100 } // 默认实现，被 @all_default_methods 覆盖
}
```

```rust
# use batch_impl::batch_impl;
// 只实现必须的，默认方法保留 trait 默认
#[batch_impl(u64 #fill(@all_required_methods){3})]
trait KeepDefault {
    fn required(&self) -> u32;
    fn optional(&self) -> u32 { 7 }
}
```

**按 receiver 种类过滤**（0.6.2 新增）：trait 方法按 receiver 形状分三类——
`&self` / `&mut self`（引用）、`self`（by-value，含 `self: Box<Self>` 等 typed
receiver）、无 receiver（关联函数 / 静态方法）：

| 标记 | 选取范围 |
|---|---|
| `@all_ref_methods` | `&self` / `&mut self` 方法 |
| `@all_value_methods` | `self`（含 typed receiver）方法 |
| `@all_static_methods` | 关联函数（无 receiver） |

典型场景是 blanket：by-value 委托语义取决于包装的 Deref/move 能力，展开期无法
区分——用 `@all_ref_methods` 只委托引用方法，by-value 方法保留 trait 默认实现：

```rust
# use batch_impl::batch_impl;
#[batch_impl(u8 { fn by_ref(&self) -> u8 { *self } })]
#[batch_impl(#blanket(@all_ref_methods){Box})]
trait RecvB {
    fn by_ref(&self) -> u8;
    fn by_val(self) -> u8 where Self: Sized { 0 }
}
// → impl<T> RecvB for Box<T> where T: RecvB {
//       fn by_ref(&self) -> u8 { (**self).by_ref() }   // 委托
//       // by_val 不生成 → Box<T> 走 trait 默认实现
//   }
// 注意：默认实现里的 `self` receiver 需要 `where Self: Sized`
```

三个标记在 `#fill` / `#delegate` / `#blanket` 与 `-` 排除中通用
（如 `#fill(@all_methods, -@all_value_methods)` = 只填引用 + 静态方法）。

### blanket 包装的 `@0` 位置标记

`#blanket` 的 `{}` 内各项（包装列表）的主部分（去掉 `where` 与 `:N`）若
**未带 `@0`**，展开为 `部分^T`（T 附加在末尾，如 `Box` → `Box<T>`）；
若**带 `@0`**，`@0` 即目标 T 的位置占位——展开为 `部分` 原样（`@0`
替换成 fresh 泛型名），T 可放在任意位置：

```rust
# use batch_impl::batch_impl;
#[batch_impl(#blanket(@all_methods){Box})]       // → Box<T>（T 在末尾）
trait PosTail { fn tag(&self) -> u32; }
#[batch_impl(#blanket(@all_methods){Box<@0>})]   // → Box<T>（等价写法）
trait PosBox { fn tag(&self) -> u32; }
# fn main() {}
```

`(u32, @0)` 同理展开为 `(u32, T)`（T 在第二位）——非 Deref 包装的委托体
需配合 where 谓词或改用 `#delegate` 自定义委托目标。

`@0` 与用户泛型自由组合：`Rc<Box<@0>>` 与 `Rc^Box` 展开等价
（`Rc<Box<T>>`）；自定义 Deref 类型带 const 参数也成立——如
`<const N: usize> #blanket(@all){MyPtrWithNum<@0, N>}` 生成
`impl<const N: usize, T> Trait for MyPtrWithNum<T, N> where T: Trait`
（用户参数 `N` 保留、`@0` 替换为 fresh 目标泛型、委托体按 Deref 深度）。

注意 blanket 的委托体仍按 deref 深度生成（`**self` 等），非 Deref 包装
（如元组）需配合 where 谓词或改用 `#delegate` 自定义委托目标。

### 列表减法 `-name`

参数中 `-` 前缀表示排除项（保留列表减去排除列表，排除优先）。
用于"批量实现除了某个 item 之外的所有项"：

```rust
# use batch_impl::batch_impl;
#[batch_impl(usize #fill(@all,-skip_me){0})]
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

`-` 后可跟标识符（`-foo`）或 `@all` 系列标记（`-@all_methods` = 排除所有方法）：
`#fill(@all,-@all_methods)` = 仅 const + type 项。也适用于 `#delegate`
（`#delegate(@all,-foo){target}`）。排除后为空、`-` 后缺目标会报 `compile_error!`。
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

> **委托限制**：`#delegate` 只支持**方法**（const / type 项报错），参数只支持
> `self` 与普通标识符（pattern 参数如 `(a, b)` 无法转发，报错）。其余限制与
> blanket 委托相同——`*const`/`*mut`、`self`、空列表均报 `compile_error!`。

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

不认识的 `#name(args){body}` 变为**顶层宏调用**：展开为 `!` 标记块
`{ ! name!{(args){body} trait ...} }`，codegen 在顶层输出该调用并把 spec 主体
前置——用户的同名宏收到 `{spec}(args){body} trait ...`（4 段，spec 主体在最前），
展开为任意 item，通常是它自己完整的 impl。这意味着指令系统是**开放的**，与
`#fill` / `#delegate` 完全同源：都是"读 trait → 生成"，只不过实现交给用户
（`#fill` 是库实现，开放指令是用户宏实现）。

```rust
# use batch_impl::batch_impl;
# use batch_impl::batch_preprocess_test; // 测试用开放扩展宏：解析 {spec}(names){body} trait → 生成 impl
#[batch_impl(usize #batch_preprocess_test(add,inc){*self+1})]
trait AddInc {
    fn add(&self) -> Self;
    fn inc(&self) -> Self;
}
// → trait AddInc { fn add(&self) -> Self; fn inc(&self) -> Self; }
// → batch_preprocess_test!{ {usize} (add,inc){*self+1} trait AddInc { fn add(&self) -> Self; fn inc(&self) -> Self; } }
//   → 宏展开为：impl AddInc for usize {
//       fn add(&self) -> Self { *self + 1 } fn inc(&self) -> Self { *self + 1 }
//     }
```

同一顶层协议也可手写：给 spec 附加 `{! m!{...}}`（`T {! m!{...}}`——用户手写
宏输入，同样 4 段）。没有 `!` 的 `T {m!{...}}` 把宏调用留在 impl body（关联项——
用户自己写完整输入含 trait）。`{!}` 块必须是 spec 的最后一个块，且至多一个。

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
#[batch_impl(#blanket(@all){&, Box, Rc})]
trait Name {
    fn name(&self) -> String;
}
// → impl Name for u32 { ... }                       // 第一个 batch_impl
// → impl<T> Name for &T    where T: Name { fn name(&self) -> String { (**self).name() } }
// → impl<T> Name for Box<T> where T: Name { ... }  // blanket 各包装一段委托
// → impl<T> Name for Rc<T>  where T: Name { ... }
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

`methods` 与 `#delegate` 相同（`@all` / `@all_methods` / 显式方法名列表）。

**包装约束谓词**：包装元素可尾随 `where{...}`（在 `:N` 之后），谓词并入
impl where 子句——解决 deref target ≠ T 的包装（如 `Cow<'_, T>` 的
deref target 是 `T::Owned`，blanket 默认委托到 T 需要额外约束）。谓词中
`@0` 指目标泛型（fresh T）、`@trait` 指本地 trait 名；`@Cow` 内置常量
即 `Cow<'_>` + 固有约束的打包：

```rust
# use batch_impl::batch_impl;
# use std::borrow::Cow;
#[batch_impl(#blanket(@all_methods){Cow<'_> where{@0: ToOwned + ?Sized, @0::Owned: @trait}})]
trait CowName { fn len(&self) -> usize; }
// → impl<T> CowName for Cow<'_, T>
//       where T: CowName, T: ToOwned + ?Sized, T::Owned: CowName
// 等价写法（内置常量）：
#[batch_impl(#blanket(@all_methods){@Cow})]
trait CowName2 { fn len(&self) -> usize; }
```

**泛型 trait 支持**（`trait Foo<X: Clone>`）：trait 形参进入 impl 泛型
（`impl<X: Clone, T> Foo<X> for 包装<T> where T: Foo<X>, ...`）——trait
形参 inline bound 经继承保留在 `<>`，`T: Trait` 与包装谓词进 where；trait
级 where 谓词透传。

**assoc type / const 委托**：`@all` 含 const/type 项时生成投影
`type Item = <T as Foo<X>>::Item;` / `const N: Ty = <T as Foo<X>>::N;`——
带必需关联类型的 trait 也能 blanket 覆盖。

```rust
# use batch_impl::batch_impl;
#[batch_impl(Foo<u32> u32 {
    type Item = u8;
    fn m(&self) -> u32 { *self }
})]
#[batch_impl(#blanket(@all){&, Box})]
trait Foo<X: Clone> {
    type Item;
    fn m(&self) -> X;
}
// → impl<X: Clone, T> Foo<X> for Box<T> where T: Foo<X> {
//     type Item = <T as Foo<X>>::Item;
//     fn m(&self) -> X { (**self).m() }
//   }
```

约束：`*const`/`*mut`（安全代码无法解引用裸指针委托）、`self`（无意义）、
空元素 / 非法 `:N` 均报错，请手写 `#delegate`。by-value receiver 方法
（`fn consume(self)`）委托语义取决于包装的 Deref/move 能力，宏展开期无法
区分——维持全放行，由 rustc 兜底。

**静态方法委托**（0.6.2）：无 receiver 的方法（`@all_static_methods` /
`@all_methods` 中的关联函数）经 blanket 泛型 `t` 转发——委托体是
`t::make(...)` 而非 deref 链（静态方法没有 `self` 可解引用）。直接调用、
嵌套包装（`Box<Box<u8>>`）、参数转发都经 `t: Trait` bound 到达底层 impl——
与 assoc item 的 `<t as Trait>::Item` 投影同一转发语义：

```rust
# use batch_impl::batch_impl;
#[batch_impl(#blanket(@all_static_methods){Box})]
trait StaticT {
    fn make() -> u8;
    fn pair(a: u8, b: u8) -> u16;
}
impl StaticT for u8 {
    fn make() -> u8 { 7 }
    fn pair(a: u8, b: u8) -> u16 { (a as u16) * 10 + b as u16 }
}
// → impl<T> StaticT for Box<T> where T: StaticT {
//       fn make() -> u8 { T::make() }
//       fn pair(a: u8, b: u8) -> u16 { T::pair(a, b) }
//   }
// 调用：<Box<u8> as StaticT>::make() → T::make() → u8::make() → 7
//      <Box<Box<u8>> as StaticT>::make() → 递归委托（Box<u8>: StaticT bound）
```

### trait 泛型实参：指令抄写时替换

`Trait<实参>` 形式的 trait 段指定具体实参——指令从 trait 定义抄签名/类型时，
trait 泛型参数会替换成实参。适合对已存在的泛型 trait 生成带固定实参的 impl：

```rust
# use batch_impl::batch_impl_only;
# struct Wrapper<T>(T);
#[batch_impl_only(
    From<bool>
    Wrapper^@u8..u16
    #from{ Wrapper(value.into()) }
)]
pub trait From<T>: Sized { fn from(value: T) -> Self; }
// → impl From<bool> for Wrapper<u8> {
//       fn from(value: bool) -> Self { Wrapper(value.into()) }
//   }
// → impl From<bool> for Wrapper<u16> { ... }
```

`From<T>` 的 `T` 在抄写 `fn from(value: T)` 时替换成实参 `bool`（body 里的 `T`
同理）。lifetime 参数/实参不参与替换——body 的 `'a` 引用 impl 自身的 lifetime。

> **注意**：fn 自身的泛型参数不要与 trait 实参重名
> （`impl<U> A<U>` 里写 `fn foo<U>`）——Rust 禁止泛型参数遮蔽，会报 E0403
> （错误会同时指向 spec 的 `<U>` 与 fn 的 `<U>`），改名即可。

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

`fn(A, B)-C` 等价于 `fn(A, B) -> C`（`-` 追加返回类型）。注意 `->` 不是
DSL 操作符——不要尝试写 `(A, B)->C`（`(` 分组后 `->` 无法解析，会报错）。

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

前缀作用于**整个列表**时自动分发到每项（`#[attr] [u8, u16]` 与
`& [u8, u16]` 都展开为每项各带前缀/修饰符的 impl）。

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
| `(Box<u8>,)^2` | `(Box<u8>, Box<u8>)`（元素可以是泛型类型）        |
| `(T1, T2)^2`  | 笛卡尔积 `(T1,T1), (T1,T2), (T2,T1), (T2,T2)` |
| `()^1..3`     | `(A,), (A, B)`（长度1到2）                    |
| `()^1..=3`    | `(A,), (A, B), (A, B, C)`（长度1到3）         |
| `(T,)^2..4`   | `(T, T), (T, T, T)`（长度2到3）               |

> 注意：`(T)` 是分组，`(T,)` 才是单元素元组——`(T)^N` 会剥离分组，等价于
> `T^N`（普通类型 `^N` 是 const 泛型实参：`(W)^2 = W<2>`，其中 `W` 为带 const
> 泛型的类型；要生成元组须写 `(T,)^N`）。`(<u8>)` 是错误语法：`(` 后直接 `<` 不是合法类型，单元素
> 元组须写完整类型加逗号，如 `(Box<u8>,)`。`(<Clone>)^N`（带 bound 的空
> 元组基座）不支持，改用 `()^N where{@0: Clone, ...}` 表达。

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

`@` 宏元层有**三个维度**：

| 维度 | 记号 | 作用 |
|------|------|------|
| **常量** | `@u*` / `@num` / `@scalar` / `@u8..u128` / 自定义 `@name=value;` | 类型族列表，解析前展开 |
| **选择器** | `@all` 族（`@all_methods` / `@all_required*` / `@all_ref_methods` / `@all_type_params` …） | 指令作用域的 item 集合选择（见 §7） |
| **位置引用** | `@N` / `@g_i` / `@all_fresh` / `@N..M` | 命名宏生成的 fresh 泛型（见下节） |

三者都是**纯词法替换**——在任何 DSL 解析前展开为 token，因此可与类型 DSL
（`[Box, Rc]^@uints`）、指令（`#fill(@all)`）、where 谓词
（`where{@0..=2: Copy}`）自由组合。

常用类型矩阵不必手写：`@` 常量在预处理阶段展开为字面列表，与手写等价。

| 常量 | 展开 |
|------|------|
| `@u*` | `[u8, u16, u32, u64, u128, usize]`（无符号族通配） |
| `@i*` | `[i8, i16, i32, i64, i128, isize]`（有符号族通配） |
| `@f*` | `[f32, f64]`（浮点族通配） |
| `@num` | `@u* + @i* + @f*`（14 个） |
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
    @uints=@u*;                      // 引用内置常量（通配族）
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

### `batch_trait!` 段级 `@trait`（跨段复用「泛型声明 + trait 名」）

`batch_trait!` 多段每段 trait 名不同——常量值里的 `@trait` 在分段后
**逐段替换为本段 trait 路径**：

```rust
# use batch_impl::batch_trait;
# trait A<T> {} trait B<T> {}
batch_trait!{
    @type_t = <T> @trait <T>;   // 打包「泛型声明 + 本段 trait 名」
    A: @type_t [&, Box]^T;      // → <T> A<T> [&, Box]^T
    B: @type_t Box^[T, Vec<T>]; // → <T> B<T> Box^[T, Vec<T>]
}
```

### 宏元层完整化：`@trait` / `@all` 系 / `@Cow` / `@0`

`batch_impl` / `batch_impl_only` 持有 trait 定义，宏元层额外提供 trait 感知
常量（`batch_trait!` 是函数式宏、拿不到定义，遇下列记号报错）：

| 记号 | 展开 | 场景 |
|---|---|---|
| `@trait` | trait 完整路径（`batch_impl`=本地名、`batch_impl_only`=外部路径）；`batch_trait!` 中为**段级**：展开为本段 trait 路径 | blanket 包装 where 谓词；`batch_trait!` 跨段打包「泛型声明+trait 名」；**顶层 spec 的 trait 名部分**（`<T> @trait<T> Vec<T>`） |
| `@all` / `@all_methods` / `@all_constants` / `@all_types` | `[item名, ...]`（Bracket 组） | 指令范围选择——`#fill(@all)` / `#fill(@all, -[a,b])` |
| `@all_required*` / `@all_default*` | 按默认实现状态过滤的 Bracket 组 | 只填必须的 / 只覆盖默认的 |
| `@all_ref_methods` / `@all_value_methods` / `@all_static_methods` | 按 receiver 类型过滤的 Bracket 组（`&self`/`&mut self` / `self` / 关联函数） | 只委托引用方法（绕开 by-value 委托语义不定）；`#blanket(@all_ref_methods){Box}` |
| `@all_type_params` / `@all_const_params` / `@all_lifetimes` | 泛型参数族：展开为**扁平 `<...>` 泛型声明**（类型参数只名字、const 完整 `const N: usize`、生命周期原样） | 泛型声明照抄 trait 形参（bound 走同名继承）；`#[batch_impl(@all_lifetimes @all_type_params Borrowed<'a, T> &'a T)]`——连续声明保持生命周期在前 |
| `@Cow` | `Cow<'_>` + 固有约束谓词 | blanket 包装（deref target = `T::Owned`） |
| `@N`（位置引用） | *本 impl* 第 N 个 fresh 泛型（`_Param_{N}_BatchGen_` 形式）的名字——每个 impl 把自身 fresh 按文档序重编号为 `0..N`，跨 spec 与 range 生成均可用 | blanket 包装谓词中 `@0` = 目标泛型（唯一 fresh）；元组生成 `()^N` 中 `@k` = 第 k 个 fresh 泛型；也可直接用于目标类型（`Box<@0>`）；**用户泛型直接写名字**（不参与 @N 索引） |
| `@g_i`（分组引用） | 第 g 个生成器的第 i 个产物（`_Param_{g}_{i}_BatchGen_`）——**跨数组分发 impl 稳定**（impl 无该组时报错而非静默漂移） | `()^3-()^3 where{@0_0: Clone}` = 左生成器第一个 fresh，`@1_0` = 右生成器第一个；也可用于目标类型 |
| `@all_fresh` | 本 impl 的全部 fresh 泛型（仅限谓词 subject） | `where{@all_fresh: Clone}` 约束全部 fresh 泛型 |
| `@N..M` / `@N..=M`（范围） | 连续 fresh 段（仅限谓词 subject） | `where{@0..=2: Copy}` 约束前三个 fresh |

> `@N` 按编号解析：where 谓词内在 codegen 阶段、目标类型内在 parse 层
> （类型域边界）；`@trait` 已提前：batch_impl 在常量阶段展开（trait 路径已知）、
> batch_trait! 在段级替换（递归进入 where 组）。

**`@N` 与 `@g_i` 怎么选**：`@N` 按每 impl 的文档序编号 fresh 泛型——简单，
但跨数组分发 impl 语义会漂移（每个 impl 从 0 重编号）。`@g_i` 指名精确的
生成点（第 g 组第 i 位）——**跨分发 impl 稳定**；分发场景下 where 谓词要
指特定生成器的 fresh 时用它（`[Box, ()^2]^()^2`）。`@all_fresh` / `@N..M`
是"全部 fresh"/"连续段"的批量形式。

**稳定性承诺**：`@N` 编号语义在 0.6.4 → 0.6.7 间修订过三次（每 impl 编号 +
文档序 + 目标类型通道）。现机制（每 impl 清扫为 `_Param_0..N_BatchGen_`、
`@N` 纯构造）视为**最终形态**——今后任何改动都按刻意破坏性发布处理。

**学习提示**：`@` 层是一个小型元语言——记号累积确有学习成本。日常只需
`@u*` / `@num` / `@scalar`（常量）+ `@all_methods`（选择器）+ `@0`
（blanket 目标）即可走得很远。分组/批量/范围引用为组合场景而存在——
谓词需要指名特定 fresh 泛型时再拿，不必提前学。

`@all` 系展开为 Bracket 组后走指令参数解析：**`#` 不再作为范围标记**——
`#` 只剩指令名一种格式（`#fill`/`#delegate`/`#blanket`/开放扩展），范围
选择统一归宏元层。减法不受影响：`#fill(@all, -foo)`、`#fill(@all, -[a,b])`。

**指令参数支持 `[a, b]` 列表**：`#fill([m1, m2]){...}`、`-` 排除也可写
`-[a, b]`（`@all` 展开产物即此形态，用户手写等价）。

**where 谓词里的 `@N` 位置引用**——宏生成的泛型名用户不知道（fresh 名），
用位置引用约束它们：

```rust
# use batch_impl::batch_impl;
// 元组生成的 fresh 泛型：@0 = 第 0 个、@1 = 第 1 个
#[batch_impl(()^2 where{@0: Clone, @1: Copy} { fn tmk() -> u32 { 2 } })]
trait TupleWhereAt { fn tmk() -> u32; }
// → impl<A: Clone, B: Copy> TupleWhereAt for (A, B) { fn tmk() -> u32 { 2 } }

// 用户泛型：直接写名字（@N 只索引宏生成的 fresh 泛型）
#[batch_impl(<T> AtWhere<T> Vec<T> where{T: Default} { fn an(&self) -> usize { self.len() } })]
trait AtWhere<T: Clone> { fn an(&self) -> usize; }
// → impl<T: Clone + Default> AtWhere<T> for Vec<T> { ... }

// 批量引用：@all_fresh 约束全部 fresh；@N..=M 约束连续段
#[batch_impl(()^3-()^3 where{@all_fresh: Clone, @0..=2: Copy})]
trait BatchWhereAt {}
```

（blanket 包装谓词中 `@0` = 目标泛型 fresh T，见 §7 `#blanket`；`@trait` 也可
出现在普通 where 谓词中，如 `where{@0: @trait<T>}`。）

**泛型参数族**（0.6.4）：泛型声明照抄 trait 形参（类型参数只名字、const 完整
声明、生命周期原样）——bound 由同名继承自动补：

```rust
# use batch_impl::batch_impl;
#[batch_impl(@all_type_params GenT<T> Vec<T> { fn head(&self) -> T { self[0].clone() } })]
trait GenT<T: Clone> { fn head(&self) -> T; }
// → impl<T: Clone> GenT<T> for Vec<T> { fn head(&self) -> T { self[0].clone() } }

#[batch_impl(@all_lifetimes @all_type_params Borrowed<'a, T> &'a T { fn get(&self) -> &'a T { *self } })]
trait Borrowed<'a, T: Clone> { fn get(&self) -> &'a T; }
// → impl<'a, T: Clone> Borrowed<'a, T> for &'a T { ... }（连续声明生命周期在前）
```

> **与 `A<>` 的关系**：`A<>`（§5）的展开**同时**包含形参声明（含 bound）和实参——
> 本身就是"全自动"（`#[batch_impl(Foo<> Vec<T>)]` 一行即声明 + 实参 + bound 全照抄）。
> `@all_type_params` 是**只自动声明**的粒度选择（实参要自定义时用）。
> **不要叠加**：`@all_type_params Foo<>` 会让两个声明源重复（rustc E0403），
> 二选一即可。

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
`[unsafe] Trait路径: impl-specs`，`:` 右侧接受类型 DSL 与 `@` 常量（与
`#[batch_impl]` 相同的类型语法），额外支持多 trait 段、路径 trait（如
`foo::C`）、unsafe 段：

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

> **限制**：`batch_trait!` **不支持 `#` 指令**（`#fill`/`#delegate`/`#blanket`/
> 开放扩展）——指令需要 trait 定义作签名真相源，而 `batch_trait!` 是函数式宏、
> 拿不到 trait 定义。需要指令时请改用 `#[batch_impl]` / `#[batch_impl_only]`。

## 13. 错误提示

所有 DSL 语法错误通过 `compile_error!()` 输出英文提示（0.6.2 起），并尽量指向
源码中出错的具体 token（span 诊断；组内 token 与 `Err` 返回路径显示宏调用行），
永不 panic：

| 错误输入 | 错误信息（节选） |
|---|---|
| `batch_trait!(;)` | `batch_trait! expects a trait name` |
| `batch_trait!(A)` | `batch_trait! expects ':' to separate the trait name and impl-specs` |
| `batch_trait!(A: B::)` | `batch_trait! expects an ident as the trait name` |
| `A^`（缺右操作数） | `batch-impl: missing operand after '^' (e.g. 'T^U')`——指向 `^` 本身 |
| `A,,B` | `batch-impl: missing operand between consecutive commas ',,`'` |
| `3..2`（空范围） | `batch-impl: range '3..2' is empty (start not below end); no impls will be generated` |
| `^2000`（超上限） | `batch-impl: tuple '^2000' expands to 2000 items (limit 1024); likely exponential/range/Cartesian typo` |
| 深度超 128 | `batch-impl: nesting depth exceeds 128 levels (perhaps an accidental extra bracket)` |
| `@unknown` | `batch-impl: unknown @ constant '@unknown'; built-ins: '@u*' ...` |
| `@u32..u8`（端点反序） | `batch-impl: range start is greater than end: 'u32..u8'` |
| `@a=@a`（循环引用） | `batch-impl: constant '@a' references unknown '@a' (undefined or defined later; ...)` |
| `#fill()`（空参数） | `batch-impl: the directive's argument list cannot be empty` |
| `-` 后缺目标 | `batch-impl: directive arguments cannot be empty` |
| 裸 `where` 缺代码块 | `batch-impl: `where` predicates are missing a code block {...}` |
| 继承谓词引用未声明形参 | `batch-impl: trait argument 'X' maps to parameter 'T' (bound 'IntoIterator'); automatic inheritance requires the same name; rename to 'T' or write the bound manually` |
| `#blanket` 非法包装 | `batch-impl: #blanket ...`（`*const`/`*mut`、`self`、空元素、非法 `:N`、无法转发的 pattern 参数均报错） |
