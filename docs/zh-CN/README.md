# batch-impl

**v0.6.8**（2026-08-08）——0.6.2–0.6.7 已发布；0.6.7：fresh 逐 impl 编号（`@N` 任意位置含目标类型）、顶层开放扩展（`{! ...}`）、`@all_fresh` / `@N..M` 批量 where 引用、错误聚合。

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
#[batch_impl(<T> Sortable<T> [Box, Rc]^Vec<T> where T: Ord  {
    fn is_sorted(&self) -> bool { self.windows(2).all(|w| w[0] <= w[1]) }
})]
trait Sortable<T> { fn is_sorted(&self) -> bool; }
// → impl<T> Sortable<T> for Box<Vec<T>> where T: Ord { ... }
// → impl<T> Sortable<T> for Rc<Vec<T>>  where T: Ord { ... }

// 一行生成单个带 4 个泛型参数的元组 impl（长度范围请用 `()^1..=4`）
#[batch_impl(()^4)]
trait TupleTrait {}
// → impl<A, B, C, D> TupleTrait for (A, B, C, D) {}
```

## 为什么要用它

为多个类型实现同一 trait，手写意味着**重复**：签名抄 N 遍、body 复制 N 份、
泛型参数与关联类型各写各的、改一处漏三处。batch-impl 把 impl 的**数量**交给人脑
之外的描述：

- **一处真相源**：trait 定义只写一次（签名/泛型/bound/where 约束），DSL 只写
  "哪些类型 × 什么实现"，其余由宏补齐——签名、泛型 bound、关联类型绑定、
  甚至 trait 级 where 约束都从 trait 定义**自动继承**，与手写完全等价。
- **一行矩阵**：`[...]` 列表、`^`/`-` 应用、`()^N` 元组生成，一条 DSL 描述
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
batch-impl = "0.6.7"
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
| 并列列表 `[A, B]`                    | 为多个类型同时实现，body 复用               | 列表与 body |
| `^` / `-` 运算符                     | 同一运算的右/左结合：嵌套与累加             | 运算符      |
| 泛型自动化                           | `A<>` 照抄、同名继承、trait where 子句继承  | 泛型自动化  |
| 关联类型绑定                         | `Iter<Item=T>` → `type Item = T;`           | 关联类型    |
| 指令系统 `#name`/`#fill`/`#delegate` | 签名自动抄、body 批量填、委托调用           | 指令系统    |
| 覆盖式委托 `#blanket`                | 包装矩阵一行生成委托 impl（任意包装 + `:N`、泛型 trait、assoc 投影、包装 where 谓词、静态方法经 `t` 转发） | 指令系统    |
| 开放扩展                             | 不认识的 `#name(args){body}` 变为顶层宏调用：你的同名宏收到 `{spec}(args){body}trait` 并生成自己的 impl | 指令系统    |
| `@` 常量                             | 内置族 `@u*`/`@scalar`/`@u8..u128` + `@trait`/`@all` 系/`@Cow` + batch_trait! 自定义（懒展开、链式引用） | 常量系统    |
| 泛型参数族                           | `@all_type_params` / `@all_const_params` / `@all_lifetimes`——泛型声明照抄 trait 形参（bound 走同名继承） | 常量系统    |
| 宏元层统一 `@`                       | `#` 只剩指令名，范围选择（`@all` 系，含 required/default 与 receiver 过滤）与位置引用（`@N`/`@g_i`/`@all_fresh`/`@N..M`）归宏元层 | 常量系统    |
| `where{...}`                         | 约束容器统一（`<>` 只留名字），blanket 约束并列合并 | where 子句  |
| 元组生成                             | `()^3`、`(T,)^N`、笛卡尔积、范围            | 元组生成    |
| fn 类型 / unsafe / 指针 / 属性       | 类型级修饰符全支持                          | 修饰符      |

## 下一步

- **完整教程**：`docs/tutorial.md`（渐进式从一行 impl 到高级矩阵组合）
- **三个入口**：`#[batch_impl]`（含 trait）/ `#[batch_impl_only]`（只出 impl）/
  `batch_trait!`（对已声明 trait 批量生成，支持多段）
- **示例**：`examples/quickstart.rs`（特性 demo）、`examples/simplify.rs`
  （29 个 impl ≈ 15 行 DSL 的真实场景）
- **开发者**：内部架构见 `docs/architecture.md`，开发变更记录见
  `docs/dev-changelog.md`

## 许可证

MIT OR Apache-2.0
