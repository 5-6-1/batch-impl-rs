# batch-impl

**v0.9.4**（2026-08-25）——**宏元层载体重建 + blanket 委托与 `#delegate` 改名增强**：`#blanket` 委托泛型关联类型（`type Iter<'a> = <T as Trait>::Iter<'a> where Self: 'a`）、裸 `Self` 参数/返回定向报错并给指导（`Self::Assoc` 返回放行）、wrapper `@?` 后缀加 `T: ?Sized`（`Box@?` → 非 Sized 目标如 `Box<dyn Trait>`）；`#delegate(size = len)` 改名委托目标方法（delegate crate 的 `#[call(...)]` 机制）——rename/`@all` 重叠合并、二次改名报错；宏元层内部重建——全部保留名替换为结构化载体（fresh 声明/引用为 `@{g_i}`、变长段模板标记为 `[Prefix; ()]` 数组形状——可编译代码中不可能存在、段槽按 `(prefix, position)` 对键控）；生成的 impl 显示可读 fresh 泛型（`P0, P1, ...`，撞名逃逸为 `P0A`、`P0B`、……——编号从不跳过；body 里的槽引用用可写的 `@{A_pos}` 拼写）；`X<>` 在 `+` 连接 bound 内同步；fresh 范围在 impl body 内重开；repeat 块新增轮间分隔符与 fresh 驱动 cursor-only 块（`impl{@0..}` + `@@N` 名称引用）。`@all_fresh` 已废弃（请写 `@0..`）。

为 Rust trait 批量生成 `impl` 块的过程宏库——**一行 DSL，展开成 N 个 impl**。

核心批量 DSL 之下还有两层更深的架构：**宏元层**（`@` 常量 / 选择器 /
位置引用——一个用于组合生成泛型的小型元语言）与**开放指令系统**
（`#fill` / `#delegate` / `#blanket` + 用户 `#name` 宏，含顶层宏注入
`{! ...}`）。可以把它看作"带可插拔 codegen 协议的批量 impl 生成器"——
"一行"故事覆盖常见场景，下面的层次覆盖组合场景（分发矩阵、blanket
委托、自定义 codegen）。

```rust
use batch_impl::batch_impl;
# use std::rc::Rc;

// 一个 body，为 4 种类型各生成一个 impl
#[batch_impl(<T> Sortable<T> [Box, Rc].Vec<T> where T: Ord  {
    fn is_sorted(&self) -> bool { self.windows(2).all(|w| w[0] <= w[1]) }
})]
trait Sortable<T> { fn is_sorted(&self) -> bool; }
// → impl<T> Sortable<T> for Box<Vec<T>> where T: Ord { ... }
// → impl<T> Sortable<T> for Rc<Vec<T>>  where T: Ord { ... }

// 一行生成单个带 4 个泛型参数的元组 impl（长度范围请用 `().1..=4`）
#[batch_impl(().4)]
trait TupleTrait {}
// → impl<A, B, C, D> TupleTrait for (A, B, C, D) {}
```

## 用 batch-impl 构建

**[alga2](https://docs.rs/alga2) 是真实用户**——现代抽象代数层次库
（[alga](https://docs.rs/alga) 的继任者，2020 起停止维护），**~900 个 impl
由 ~80 条 batch-impl DSL 生成**，覆盖 15+ 类型（数值、元组 1–16、数组、
`Option`、`Complex`、`Quaternion`、`ModN`、智能指针、集合）。**alga2 0.1.0
已在 crates.io 发布**；开发全程以 batch-impl DSL 为 impl 生成器。

## 为什么要用它

为多个类型实现同一 trait，手写意味着**重复**：签名抄 N 遍、body 复制 N 份、
泛型参数与关联类型各写各的、改一处漏三处。batch-impl 把 impl 的**数量**交给人脑
之外的描述：

- **一处真相源**：trait 定义只写一次（签名/泛型/bound/where 约束），DSL 只写
  "哪些类型 × 什么实现"，其余由宏补齐——签名、泛型 bound、关联类型绑定、
  甚至 trait 级 where 约束都从 trait 定义**自动继承**，与手写完全等价。
- **一行矩阵**：`[...]` 列表、`.`/空格 应用、`().N` 元组生成，一条 DSL 描述
  "类型矩阵"，宏对每个格子生成一个 impl。
- **批量但不失手写质感**：`{ body }` 是普通 Rust 代码，`#` 指令自动抄签名，
  生成的 impl 与手写逐 token 等价——rustc 能验证什么，它就能验证什么。

一个真实场景（见 `examples/simplify.rs`）：12 个数值类型 + 4 个包装类型 +
4 个元组 + 若干杂项 = **29 个 impl，约 15 行 DSL**，手写约 80 行。

## 心智模型

你写的是**一条"类型矩阵"的描述**，batch-impl 对矩阵的每个格子生成 impl：

```text
#[batch_impl( <impl-泛型> Trait名<trait-泛型> 目标类型矩阵 { body }? )]
```

| 记号      | 含义                                  | 直觉                         |
|-----------|---------------------------------------|------------------------------|
| `.` / 空格 | 应用：把左侧容器/修饰符作用到右侧类型 | **同一个运算**，仅结合性不同 |
| `[A, B]`  | 列表                                  | 横向展开（笛卡尔积）         |
| `(A, B)`  | 元组                                  | 排列（有序对）           |           
| `*[...]` / `*(...)` | splat：摊平进外层列表 | `[a, *[b,c]]` = `[a,b,c]`；左操作数 `*[...]` 分配 / `*(...)` 追加 |
| `#name`   | 指令：从 trait 定义自动抄 item 签名   | body 不用手写签名            |

**空格是主推的写法**（左侧是修饰符/容器/trait，右侧是目标类型，链式累加参数，左结合）：`HashMap u32 String` = `HashMap<u32, String>`，`fn(A, B) C` = `fn(A, B) -> C`，`Tr u8` = `impl Tr for u8`（裸 trait 名按 impl trait 应用；要类型 `Tr<u8>` 直接写 `Tr<u8>`）。

`. ` 是同一运算的**右结合**形态，只在需要**嵌套**时用：`Box.Box u8` = `Box<Box<u8>>`，`HashMap<K> String` = `HashMap<K, String>`（空格同样可以）。

`[A, B] [X, Y]` = 2×2 矩阵（4 个 impl）；`(T1, T2).2` = 排列（4 个有序对）。

## 快速开始

```toml
[dependencies]
batch-impl = "0.8.1"
```

需要 Rust 2024 edition 及以上。

```rust
use batch_impl::batch_impl;

// 1. 定义 trait，方法签名只写一次
trait Describe { fn describe(&self) -> String; }

// 2. 写一条 DSL：目标类型 + body（方法签名用 #name 自动从 trait 抄）
#[batch_impl(
    [usize, isize] #name{"number"},
    String #name{"string"}
)]
trait Tagged { fn name(&self) -> &str; }
// → impl Tagged for usize  { fn name(&self) -> &str { "number" } }
// → impl Tagged for isize  { fn name(&self) -> &str { "number" } }
// → impl Tagged for String { fn name(&self) -> &str { "string" } }

// 3. 0.6.2：一行 blanket——为所有包装类型生成委托 impl
//    （实例方法经 deref 转发；@all_ref_methods 只选引用方法，by-value 走默认）
# use std::rc::Rc;
#[batch_impl(#blanket(@all_ref_methods){&, Box, Rc})]
trait Describe2 { fn describe(&self) -> String; }
// → impl<T> Describe2 for &T    where T: Describe2 { fn describe(&self) -> String { (**self).describe() } }
// → impl<T> Describe2 for Box<T> where T: Describe2 { ... }
// → impl<T> Describe2 for Rc<T>  where T: Describe2 { ... }
```

## 特性一览

| 特性                                 | 一句话                                      | 教程章节    |
|--------------------------------------|---------------------------------------------|-------------|
| 并列列表 `[A, B]`                    | 为多个类型同时实现，body 复用               | §3 |
| splat `*` 前缀                      | 摊平容器/生成器进外层列表——列表内拼接、`.` 右操作数扁平追加、泛型多实参；左操作数 `*[...]` 分配 / `*(...)` 追加 | §4 |
| `.` / 空格 运算符                  | 同一运算的右/左结合：嵌套与累加             | §2 |
| 泛型自动化                           | `A<>` 照抄、同名继承、trait where 子句继承  | §5 |
| 关联类型绑定                         | `Iter<Item=T>` → `type Item = T;`           | §5.3 |
| 指令系统 `#name`/`#fill`/`#delegate` | 签名自动抄、body 批量填、委托调用           | §7 |
| 覆盖式委托 `#blanket`                | 包装矩阵一行生成委托 impl（任意包装 + `:N`、泛型 trait、assoc 投影、包装 where 谓词、静态方法经 `t` 转发） | §7 |
| 开放扩展                             | 不认识的 `#name(args){body}` 变为顶层宏调用：你的同名宏收到 `{spec}(args){body}trait` 并生成自己的 impl | §7 |
| `@` 常量                             | 内置族 `@u*`/`@scalar`/`@u8..u128` + `@trait`/`@all` 系/`@Cow` + `batch_trait!` 前导自定义段 `@name=value;`（懒展开、链式引用；属性宏不支持——矩阵直接写） | §6 |
| 泛型参数族                           | `@all_type_params` / `@all_const_params` / `@all_lifetimes`——泛型声明照抄 trait 形参（bound 走同名继承） | §6 |
| 宏元层统一 `@`                       | `#` 只剩指令名，范围选择（`@all` 系，含 required/default 与 receiver 过滤）与位置引用（`@N`/`@g_i`/`@all_fresh`/`@N..=M`）归宏元层 | §6 |
| `where{...}`                         | 约束容器统一（`<>` 只留名字），blanket 约束并列合并 | §8 |
| 元组生成                             | `().3`、`(T,).N`、笛卡尔积、范围            | §9 |
| 变长段 + 重复块                      | `impl{...}` 模板内 `ident@..`（覆盖所有剩余元组位置）+ body 内 `@(...)..` 重复（`@ident` 名字、`@N` 索引游标）——一条 spec 覆盖所有元组 arity | §8.4 |
| fn 类型 / unsafe / 指针 / 属性       | 类型级修饰符全支持（`unsafe fn` 是 fn 类型；`unsafe.fn` 才是 unsafe impl 标记） | §10 |

> **简写提示**：单方法 `#fill([foo]){body}` 等价于 `#foo{body}`；谓词 + 代码块 `where{谓词} {代码块}` 可裸写成 `where 谓词 {代码块}`（详见 §7.2 / §8.2）。

## 语法面冻结承诺（0.7.2 起）

全部既有记号的语义视为 **final**——`.`/空格、`[]`/`()`/`<>`、`where`、
`#` 指令、`@` 常量、splat 的既有行为不再改变；后续版本只做**加法**（新指令 /
新常量 / 新工具）、诊断精化与文档。任何对既有语义的改动都是刻意的破坏性发布
（`@N` 的稳定性承诺自此推广到整个语法面）。`@g_i` / `@all_fresh` / `@N..M`
属 power-user tier（见 tutorial §6.4），新手从 `@u*` / `@all_methods` / `@0` 起步。

## 下一步

- **完整教程**：`docs/tutorial.md`（渐进式从一行 impl 到高级矩阵组合）
- **三个入口**：`#[batch_impl]`（含 trait）/ `#[batch_impl_only]`（只出 impl）/
  `batch_trait!`（对已声明 trait 批量生成，支持多段）
- **impl entry / shape template（0.8.0）**：**ItemImpl 入口**——`#[batch_impl]` 同样接受 `impl` 块，按形状模板 × 矩阵源批量实例化（教程 §8.5）；**`impl{...}` Self-part 形状模板**——绑定生成 impl 的目标形状，**每个形状族写一个原型实现**即可覆盖整个矩阵，含 `Cow` 这类含生命周期的族（教程 §8.4）
- **变长段 + 重复块（0.8.2）**：模板 `ident@..` 段 + body `@(...)..` 重复——alga2 风格 `().1..=4 where @0..: Magma impl{(A@..)} #combine{...}` 一条 spec 覆盖所有元组 arity（教程 §8.4）
- **展开预览**：`batch_preview!`（把 `#[batch_impl(...)] trait` / `#[batch_impl(...)] impl` 原样包进去，展示真实展开 +
  `.`/空格 结合性误写提示）
- **示例**：`examples/quickstart.rs`（特性 demo）、`examples/simplify.rs`
  （29 个 impl ≈ 15 行 DSL 的真实场景）、`examples/typeclass.rs`
  （类型类风格：`Num`/`UNum`/`INum`/`FNum` 层级 + `Frac<T, U>` 的 36 个 `From<bool>` impl）
- **开发者**：内部架构见 `docs/architecture.md`，开发变更记录见
  `docs/dev-changelog.md`

## 许可证

MIT OR Apache-2.0
