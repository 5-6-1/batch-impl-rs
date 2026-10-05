# batch-impl

仓库源码（GitHub `main`）：[English](https://github.com/5-6-1/batch-impl-rs/blob/main/README.md) | 简体中文

**v0.10.1。** 破坏性变更与从 0.9.7 迁移的说明见 [CHANGELOG](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/CHANGELOG.md)。

仓库源码链接打开公开的 `main`，可能尚未包含本地修改。在本地运行
`cargo doc --no-deps --open` 可阅读当前工作树的英文文档；其顶部导航留在
本次构建版本内。中文教程与参考手册请在同一源码目录中阅读。

为类型列表或类型矩阵批量生成 Rust trait 实现的过程宏库。

## 为什么要用它

- **把相关实现放在一起维护。** 写明哪些类型共享一个方法体；修改方法体时，整组实现一起更新。
- **从自己的 trait 复制签名。** `#方法名{body}` 自动补齐对应方法的签名，实现中只需提供方法体。
- **从列表扩展到类型族。** 有需要时再加入泛型容器、元组长度或包装器委托。生成的实现是普通 Rust，由 rustc 检查。

已有普通 Rust `impl` 时，可以直接[批量复用这个实现](#批量复用已有-impl)，包括外部 trait，无需签名镜像。需要复制签名、填充或委托指令时，本地 trait 由被标注的定义提供签名，外部 trait 则通过 `batch_impl_only` 提供并维护镜像；见[入口选择](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/tutorial.md#11-入口)。

## 快速开始

需要 **Rust 1.95 或更新版本**。本页描述 **0.10.1 源码树**。如果想使用已发布的 0.9.7，请阅读它的[版本文档](https://docs.rs/batch-impl/0.9.7/batch_impl/)。

试用当前源码时，在源码目录旁创建一个小程序：

```text
work/
  batch-impl/    # this source checkout
  demo/         # your new application
```

在 `work/` 下运行 `cargo new demo`。打开 `demo/Cargo.toml`，把已有的空 `[dependencies]` 段替换为：

```toml
[dependencies]
batch-impl = { path = "../batch-impl" }
```

请使用包含待试用改动的源码目录。路径依赖能直接使用本地未提交的改动，Git 依赖无法获取这些改动。目前没有已发布的 `batch-impl = "0.10.0"` 版本。

将下面的完整程序复制到 `demo/src/main.rs`：

```rust
use batch_impl::batch_impl;

#[batch_impl([u8, u16, u32] #describe{format!("number: {self}")})]
trait Describe {
    fn describe(&self) -> String;
}

fn main() {
    assert_eq!(7u8.describe(), "number: 7");
    assert_eq!(12u16.describe(), "number: 12");
    assert_eq!(300u32.describe(), "number: 300");
    println!("{}", 7u8.describe());
}
```

在 `demo/` 下运行 `cargo run`。三个断言全部通过，程序输出 `number: 7`。

缩减目标时，单个类型写 `u8`，单元素列表写 `[u8,]`；**`[u8]` 表示切片类型**。列表中的尾逗号可以保留。

`[u8, u16, u32]` 选中三种类型。`#describe{...}` 复制 `Describe::describe` 的签名，为每个实现填入相同的方法体。`#` 后面是你的 trait 成员名，不是固定关键字。三个生成的实现之一是：

```text
impl Describe for u8 {
    fn describe(&self) -> String { format!("number: {self}") }
}
```

## 批量复用已有 impl

如果已经写好了一个实现，在它上面加属性即可。下面是另一个完整程序：

```rust
use batch_impl::batch_impl;

struct UserId(u64);
struct OrderId(u64);

#[batch_impl(@Self: [UserId, OrderId])]
impl std::fmt::Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "id:{}", self.0)
    }
}

fn main() {
    assert_eq!(UserId(7).to_string(), "id:7");
    assert_eq!(OrderId(12).to_string(), "id:12");
}
```

`@Self` 指输入 impl 的自身类型（这里是 `UserId`），右侧列出生成实现的目标。生成结果替换原 impl，因此列表包含 `UserId` 才会保留它的实现。这个入口直接复用完整方法，不需要 `#fmt` 或 `Display` 的签名镜像。

复用代码中的字段、方法、构造与约束需要对每个目标成立；例如这里两种类型都有可显示的 `.0` 字段。详细规则见[教程的入口选择](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/tutorial.md#11-入口)。

## 沿着同一个 trait 继续

回到快速开始的 `Describe`，将那个程序替换为这个扩展示例。它加入泛型 `Vec<T>` 实现，并让 `Box<T>` 委托给实现了 `Describe` 的内部 `T`：

```rust
use batch_impl::batch_impl;

#[batch_impl(
    [u8, u16, u32] #describe{format!("number: {self}")},
    <T> Vec<T> #describe{format!("{} items", self.len())},
    #blanket(@all_ref_methods){Box}
)]
trait Describe {
    fn describe(&self) -> String;
}

fn main() {
    assert_eq!(7u8.describe(), "number: 7");
    assert_eq!(vec![1, 2, 3].describe(), "3 items");
    assert_eq!(Describe::describe(&Box::new(7u8)), "number: 7");
}
```

`<T> Vec<T>` 为这个 impl 声明泛型。`#blanket` 生成 `impl<T: Describe> Describe for Box<T>`，引用接收者方法转发到内部值。最后一个断言显式调用这个包装器实现。

某个类型需要不同实现时，把它拆成独立 spec，再让其余类型共享 body。**局部 body 与共享 body 合并，不会互相覆盖**；重复提供同名方法会报错。[连续练习](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/tutorial.md#1-实现并调用一个方法)逐步演示加类型、加方法、特殊实现、泛型约束与包装转发。

对于自己定义的包装器，`#delegate` 可以经字段转发；对于内部类型不同的枚举，`inner.#call` 可以在每个分支中转发。继续阅读[委托教程](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/tutorial.md#7-指令系统-)。

## 结果不符合预期时

临时把属性和它标注的 trait 或 impl 一起放入 `batch_impl::batch_preview!`：

```text
batch_impl::batch_preview! {
    #[batch_impl([u8, u16, u32] #describe{format!("number: {self}")})]
    trait Describe { fn describe(&self) -> String; }
}
```

运行 `cargo check`，在诊断中阅读生成的 Rust。**预览工具会故意通过编译错误显示输出。** 将结果与预期实现对照，修改 DSL，再去掉预览包装，正常编译。[预览工具](https://github.com/5-6-1/batch-impl-rs/blob/main/src/doc/batch_preview.md)也给出了 `batch_impl_only` 和普通 impl 入口的完整例子。

例如，`Box.Vec u32` 展开为 `Box<Vec, u32>`。Rust 可能提示缺少泛型参数或涉及 `allocator_api`，诊断不保证展示完整的生成类型。要表达嵌套，可以写 `Box (Vec u32)`、`Box.Vec.u32`，或普通 Rust 类型 `Box<Vec<u32>>`。

## 阅读类型表达式

先使用普通 Rust 类型和列表。空格从左到右累积参数，`.` 从右边结合以表达嵌套：

| 写法 | 含义 |
|---|---|
| `[u8, u16]` | 为每种类型生成一个实现 |
| `Vec u8` | `Vec<u8>` |
| `HashMap u32 String` | `HashMap<u32, String>` |
| `Box (Vec u8)` / `Box.Vec.u8` | `Box<Vec<u8>>` |
| `Box.Vec u32` | `Box<Vec, u32>` —— `.` 先结合，所以后面的空格是**第二个**实参（三个探针都把它读成了笔误） |
| `[Box, Vec] [u8, u16]` | 四种容器与类型组合 |
| `().3` | 一个泛型三元组实现 |
| `(*Vec *[].3,)` | 一个元组，各成员为独立生成的 `Vec<P0>`、`Vec<P1>`、`Vec<P2>`（fresh 泛型名为 `P0…`） |

空格保持左结合，`.` 保持右结合且优先于空格；括号可直接表明分组。[教程](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/tutorial.md)通过例子逐步展开这些规则，[参考手册](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/reference.md)记录完整边界。

## 功能概览

第一条学习路线是列表与共享方法体，再学泛型、约束和委托。其余能力按需阅读，不要求一开始就采用紧凑写法。

| 功能 | 用途 | 教程源码（GitHub main） |
|---|---|---|
| 列表与 `#方法名{body}` | 给多个类型提供同一个实现体 | [§1](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/tutorial.md#1-实现并调用一个方法)、[§3](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/tutorial.md#3-列表与-body) |
| 普通 impl 入口 | 将已经写好的实现用于多个目标 | [§11](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/tutorial.md#11-入口) |
| 空格 / `.` 与括号 | 应用泛型参数、嵌套容器 | [§2](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/tutorial.md#2-类型矩阵空格与-) |
| 泛型继承与关联类型 | 复用 trait 参数、约束和关联类型绑定 | [§5](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/tutorial.md#5-泛型-从声明到可编程实参) |
| `where` | 为一组实现添加约束 | [§8.1–§8.3](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/tutorial.md#8-where-子句) |
| `#fill`、`#delegate`、`#blanket` | 填充多个成员，或把方法转发给内部类型 | [§7](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/tutorial.md#7-指令系统-) |
| 包 `*` | 对成员映射构造规则，并拼入实参 | [§4](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/tutorial.md#4-包-映射与拼入) |
| `@` 常量与位置引用 | 选择类型族或引用生成的参数 | [§6](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/tutorial.md#6--常量系统宏元层) |
| 元组长度与笛卡尔幂 | 生成元组族和类型组合 | [§9](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/tutorial.md#9-元组生成与矩阵) |
| 形状模板 | 实例化嵌套类型的实现模式 | [§8.4](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/tutorial.md#84-impl-shape-template-形状模板080) |
| 类型修饰符 | 引用、指针、函数类型、属性与 unsafe impl | [§10](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/tutorial.md#10-修饰符大全) |
| 开放指令与重复块 | 扩展生成方式，或沿元组位置重复方法体 | [§7](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/tutorial.md#7-指令系统-)、[§8.4](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/tutorial.md#84-impl-shape-template-形状模板080) |

团队代码优先选择容易看出生成结果的写法。如果命名类型、显式泛型参数或括号更能表达任务，就使用它们，无需追求最短的矩阵表达式。

## 使用 batch-impl 的项目

[alga2](https://docs.rs/alga2) 使用 batch-impl 为数值、元组、数组、智能指针等类型构建抽象代数层级。仓库的 [simplify 示例](https://github.com/5-6-1/batch-impl-rs/blob/main/examples/simplify.rs)展示了较小的完整场景：**约 15 行 DSL 生成 30 个实现**。

当许多实现遵循相同规则、需要协调修改时，这个库最有价值。对于少量相互独立的实现，可以比较重复劳动与团队学习成本。通过 `batch_impl_only` 使用外部 trait 时，也要计入维护签名镜像的成本。

## 展开开销

宏在编译期运行。`cargo test --lib perf -- --nocapture` 测量展开管线，不包含 rustc 的类型检查。在作者机器上，9 次 stable Rust 运行中，1024 个 impl 的笛卡尔规格测得 **0.10–0.20 ms/impl**，典型的 4 个 impl 规格为 **0.6–2.6 ms**。这些是观测值，不是性能保证；运行测试会输出当前测量结果。

## 兼容性与迁移

已有 token 语义受 0.7.2 引入的语法冻结承诺保护。兼容版本增加能力、改善诊断，并修复与已记录规则相矛盾的行为；刻意的语法变更必须设置兼容性边界并说明迁移方法。

本轮开发目标是 **0.10.0**，取代此前计划的 0.9.8，因为包含刻意的破坏性改动。Cargo 的 `"0.9.7"` 依赖约束允许升级到 0.9.8，却不允许升级到 0.10.0；见 [Cargo 版本规则](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#default-requirements)。

- 将已移除的 `@all_fresh` 替换为 `@0..`。
- `*` 现在只打开候选列表：映射两个构造器要写 `*[F,G] T`。元组是类型，所以 `*(F,G)` 是一个成员——该元组——`*(F,G) T` 则把 `T` 追加进去。单独的包不再将分组升格为容器，需要容器时写 `(*X,)` / `[*X,]`。见[包的迁移规则](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/reference.md#46-分支重叠与迁移)。
- 重命名原来叫 `Self` 的自定义常量；`@Self` 现在保留给 impl 入口的输入自身类型。
- 命名类型族范围采用 Rust 的端点规则：`@u8..u16` 只选择 `u8`；要保留两种类型，使用 `@u8..=u16`。省略上界时仍包含族的**最大值** —— `@u16..` 就是 `@u16..=u128`，而 `usize` 与 `isize` 都不属于任何范围族。

完整迁移记录见 [CHANGELOG](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/CHANGELOG.md)。`main` 上的源码和文档描述正在开发的内容；维护已发布版本时，应阅读对应版本的文档。

**如何自查你的迁移。** 两个测量就能覆盖几乎全部问题，而且都很便宜：

- **数 impl**：`cargo expand --lib | grep -c 'impl.* for '`，或针对单个 trait 数（`cargo expand --lib | grep -c 'impl.*EuclideanDomain'`），与旧版本对比。有意的边界变更表现为**少一个 impl** 而不是报错——最常见的原因是范围（`@u8..u64` 现在**不含** `u64`，`@u8..=u64` 才含），而失败通常在很远的地方以 `the trait bound … is not satisfied` 的形式出现。
- **读展开**：把 spec 包进 `batch_preview!`，它会以 `compile_error!` 文本打印**生成了多少个 impl** 以及每个目标类型，因此"这条 spec 生成的 impl 比我想要的少"在下游崩之前就能看见。

两项检查都不需要先发版：直接在你要迁移的那棵树上跑即可。

## 下一步

- [教程](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/tutorial.md)：从一个有用的实现开始，再按任务选择阅读路线。
- [参考手册](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/reference.md)：语法规则、合法位置、诊断与限制。
- [可运行的 quickstart](https://github.com/5-6-1/batch-impl-rs/blob/main/examples/quickstart.rs)、[simplify](https://github.com/5-6-1/batch-impl-rs/blob/main/examples/simplify.rs)、[typeclass](https://github.com/5-6-1/batch-impl-rs/blob/main/examples/typeclass.rs)：完整示例，使用 `cargo run --example quickstart` 运行（或替换为其他示例名）。
- [架构](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/architecture.md)、[开发指南](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/development-guide.md)、[开发记录](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/dev-changelog.md)：贡献者文档。

## 许可证

MIT OR Apache-2.0
