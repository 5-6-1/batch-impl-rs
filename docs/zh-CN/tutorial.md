# batch-impl 教程

**v0.10.0 — 开发中（未发布）。** 本教程描述开发中的当前工作树。待发布改动与迁移说明见 [CHANGELOG](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/CHANGELOG.md)。

**仓库源码（GitHub main）**：简体中文 · [English](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/tutorial.md) · [README](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/README.md) · [参考手册](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/reference.md)

上方及文中的 GitHub 链接用于查看仓库源码；公共 `main` 可能尚未同步当前
本地工作树。本教程内部的章节跳转留在当前页面。

先为一个类型实现方法，再用五次修改扩展同一个程序。宏生成的始终是
普通 Rust impl；已经有完整 impl 时，也可以直接从它开始。

## 0. 按任务选择阅读路线

第一次阅读，完成 [§1 的连续任务](#1-实现并调用一个方法)：
加类型、加方法、为特殊类型拆 spec、加入泛型约束、转发包装器方法。
已经写好普通 impl，或正在实现外部 trait 时，可以先看
[§1.6 的 impl 入口](#16-从普通-display-impl-开始)。
这两条都是常用路线，不需要先学形状模板或生成器。

后面的章节按主题查阅：先掌握普通类型与泛型，再在确实需要一次填入多个
类型实参或逐位置包装时看参数包。基础签名复制与简单委托已经在任务中出现；完整的
指令规则、生成器与自定义扩展可以按需阅读，不必按编号通读。

按照 [README](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/README.md)
配置依赖后，§1 每一步的完整程序都可直接替换 `src/main.rs`，然后运行
`cargo run`。§1.1–§1.5 保留前一步行为，只引入该步所讲的变化；§1.6 是另一条
入口路线的独立程序。§2 以后的代码块是独立
示例；部分使用 `# ` 开头的行放置 rustdoc 隐藏的辅助代码。复制 Markdown
源码时，去掉这个前缀、保留该行其余内容；这是文档约定，不是 DSL 语法。
英文 Rust 代码块除标为 `ignore` 的示例外均作为 doctest，下面的诊断演示
使用 `text` 代码块。

| 我想…… | 看 | 层级 |
|---|---|---|
| 完成一个逐步扩展的程序 | [§1](#1-实现并调用一个方法) | 基础 |
| 直接批量化普通 impl，包括外部 trait 的 impl | [§1.6](#16-从普通-display-impl-开始) | 基础 |
| 给已有实现加类型、修改签名、拆出例外 | [§1.7](#17-维护同一份-describe) | 按需 |
| 用包装矩阵（空格、`.`、列表）覆盖一批类型 | [§2](#2-类型矩阵空格与-) | 基础 |
| 合并独立与共享 body | [§3](#3-列表与-body) | 基础 |
| 声明泛型、继承或添加 bound、写限定类型 | [§5](#5-泛型-从声明到可编程实参) | 基础 |
| 用标准 Rust 的 `where` 约束 | [§8.1–§8.3](#81-where-谓词) | 基础 |
| 查指令的完整规则 | [§7](#7-指令系统-) | 按需 |
| 映射成员并拼入实参（`*`） | [§4](#4-包-映射与拼入)，建议先读泛型 | 按需 |
| 生成元组、各元数与笛卡尔矩阵 | [§9](#9-元组生成与矩阵) | 按需 |
| 用引用、指针、`unsafe`、属性、`!`、`self` | [§10](#10-修饰符大全) | 按需 |
| 在几个入口宏之间选 | [§11](#11-入口) | 按需 |
| 搞清某个错误是什么意思 | [§12](#12-错误提示) | 按需 |
| 读一个完整真实文件 | [§13](#13-实战仓库里那三个示例) | 按需 |
| 寻址生成的参数（`@N` / `@g_i` / 区间） | [§6](#6--常量系统宏元层) | 进阶 |
| 用形状模板、分阶段替换扩展 impl | [§8.4–§8.5](#84-impl-shape-template-形状模板080) | 进阶 |

## 1. 实现并调用一个方法

先让 `u8` 拥有一个可调用的 `describe` 方法：

```rust
use batch_impl::batch_impl;

#[batch_impl(u8 #describe{format!("number: {self}")})]
trait Describe {
    fn describe(&self) -> String;
}

fn main() {
    assert_eq!(7u8.describe(), "number: 7");
    println!("{}", 7u8.describe());
}
```

运行 `cargo run`：断言通过，程序输出 `number: 7`。`u8` 是目标类型；
`#describe{...}` 把 `Describe::describe` 的签名复制到 impl 并提供方法体，
`self` 就是接收调用的值。

`#` 后面是 trait 成员的实际名字。`#describe` 填写 `describe`；文档里的
`#name{body}` 表示“这里写成员名”，并不是固定的 `name` 关键字。
trait 本身仍然存在，生成的方法按普通 Rust 方式调用。

### 1.1 加类型

把单个 `u8` 换成列表 `[u8, u16, u32]`，为新增类型补上调用断言。
每一步都给出完整程序；用它替换上一版，再运行 `cargo run`。

```rust
use batch_impl::batch_impl;

#[batch_impl(
    [u8, u16, u32] #describe{format!("number: {self}")}
)]
trait Describe {
    fn describe(&self) -> String;
}

fn main() {
    assert_eq!(7u8.describe(), "number: 7");
    assert_eq!(8u16.describe(), "number: 8");
    assert_eq!(9u32.describe(), "number: 9");
    println!("{}", 7u8.describe());
}
```

缩回一个目标时，写 `u8` 或 `[u8,]`。**类型位置的 `[u8]` 是切片类型，
不是只有 `u8` 的列表**；`[u8; 3]` 则是定长数组。这个区别与指令中的
成员选择列表不同：例如 `#fill([describe])` 不需要尾逗号。

### 1.2 加方法

给同一个 trait 增加 `kind`，再用 `#kind` 提供它的实现。原来的三个
`describe` 实现保持不变，两个方法都能按普通 Rust 方式调用。

```rust
use batch_impl::batch_impl;

#[batch_impl(
    [u8, u16, u32] #describe{format!("number: {self}")} #kind{"integer"}
)]
trait Describe {
    fn describe(&self) -> String;
    fn kind(&self) -> &'static str;
}

fn main() {
    assert_eq!(7u8.describe(), "number: 7");
    assert_eq!(8u16.describe(), "number: 8");
    assert_eq!(9u32.describe(), "number: 9");
    assert_eq!(7u8.kind(), "integer");
    println!("{}", 7u8.describe());
}
```

这里两个指令分别补齐两个成员。方法体只有返回表达式，签名仍由 trait 提供。
完整写法见 [§7.1](#71-namebody--单-item-赋值)。

### 1.3 特殊类型用独立 spec

`bool` 需要不同的描述和类别，因此为它写一个独立 spec，与数值 spec 用
逗号分隔。不要把它放进带共享 `describe` 的列表后，再期待局部 body 覆盖
同名方法：**body 合并是追加成员，不是覆盖；重复定义同一成员会由 Rust 报错。**

```rust
use batch_impl::batch_impl;

#[batch_impl(
    [u8, u16, u32] #describe{format!("number: {self}")} #kind{"integer"},
    bool #describe{format!("bool: {self}")} #kind{"boolean"}
)]
trait Describe {
    fn describe(&self) -> String;
    fn kind(&self) -> &'static str;
}

fn main() {
    assert_eq!(7u8.describe(), "number: 7");
    assert_eq!(8u16.describe(), "number: 8");
    assert_eq!(9u32.describe(), "number: 9");
    assert_eq!(7u8.kind(), "integer");
    assert_eq!(true.describe(), "bool: true");
    assert_eq!(true.kind(), "boolean");
    println!("{}", 7u8.describe());
}
```

整数共用一份实现，`bool` 使用另一份，两组目标没有重叠。独立 body 与共享
body 可以添加不同成员；合并的完整例子见
[§3](#3-列表与-body)。

### 1.4 加入泛型约束

现在给 `Vec<T>` 增加一条 spec。`<T: std::fmt::Debug>` 声明 impl 的泛型
和约束；`format!("{self:?}")` 需要元素实现 `Debug`，所以这个约束有实际用途。
已有整数和 `bool` 的实现继续保留。

```rust
use batch_impl::batch_impl;

#[batch_impl(
    [u8, u16, u32] #describe{format!("number: {self}")} #kind{"integer"},
    bool #describe{format!("bool: {self}")} #kind{"boolean"},
    <T: std::fmt::Debug> Vec<T> #describe{format!("{self:?}")} #kind{"list"}
)]
trait Describe {
    fn describe(&self) -> String;
    fn kind(&self) -> &'static str;
}

fn main() {
    assert_eq!(7u8.describe(), "number: 7");
    assert_eq!(8u16.describe(), "number: 8");
    assert_eq!(9u32.describe(), "number: 9");
    assert_eq!(7u8.kind(), "integer");
    assert_eq!(true.describe(), "bool: true");
    assert_eq!(true.kind(), "boolean");
    assert_eq!(vec![1u8, 2].describe(), "[1, 2]");
    assert_eq!(vec![1u8, 2].kind(), "list");
    println!("{}", 7u8.describe());
}
```

这条 spec 生成 `impl<T: std::fmt::Debug> Describe for Vec<T>`。
声明使用普通 Rust bound；无需使用自动生成参数或位置引用。普通泛型的其余规则见
[§5](#5-泛型-从声明到可编程实参)。

### 1.5 转发 Box 方法

最后加一条 `#blanket(@all_ref_methods){Box}`，让 `Box<T>` 的两个引用接收者
方法都转发给内部 `T`。它要求 `T: Describe`，因此之前的整数、`bool` 和
`Vec<T>` 都能继续包装。

```rust
use batch_impl::batch_impl;

#[batch_impl(
    [u8, u16, u32] #describe{format!("number: {self}")} #kind{"integer"},
    bool #describe{format!("bool: {self}")} #kind{"boolean"},
    <T: std::fmt::Debug> Vec<T> #describe{format!("{self:?}")} #kind{"list"},
    #blanket(@all_ref_methods){Box}
)]
trait Describe {
    fn describe(&self) -> String;
    fn kind(&self) -> &'static str;
}

fn main() {
    assert_eq!(7u8.describe(), "number: 7");
    assert_eq!(8u16.describe(), "number: 8");
    assert_eq!(9u32.describe(), "number: 9");
    assert_eq!(7u8.kind(), "integer");
    assert_eq!(true.describe(), "bool: true");
    assert_eq!(true.kind(), "boolean");
    assert_eq!(vec![1u8, 2].describe(), "[1, 2]");
    assert_eq!(vec![1u8, 2].kind(), "list");
    let boxed = Box::new(vec![1u8, 2]);
    assert_eq!(Describe::describe(&boxed), "[1, 2]");
    assert_eq!(Describe::kind(&boxed), "list");
    println!("{}", 7u8.describe());
}
```

两个 `Describe::...(&boxed)` 调用显式验证 `Box` 自己的 trait 实现，
避免普通方法调用的自动解引用掩盖缺失的包装器实现。这一步没有修改 trait，
也没有再写两个方法的签名。其他接收者和关联项的边界见
[§7.4](#74-blanketmethods包装列表--覆盖式委托)。

**学到这里就可以开始使用。** 列表、成员 body、独立 spec、普通泛型和
简单转发已经能覆盖许多批量实现。已有完整 impl 时看 [§1.6](#16-从普通-display-impl-开始)，
后续维护看 [§1.7](#17-维护同一份-describe)；其余章节按需要查阅。

书写时可以直接使用空格应用（`Vec u8`）；嵌套用括号写清楚，如
`Box (Vec u8)`，点号写法 `Box.Vec.u8` 也可用。每个 spec 单独一行，
较长的方法体再分行缩进。`rustfmt` 不保证按这套 DSL 的语义整理宏参数，
矩阵的分组与布局仍需自己检查。

### 1.6 从普通 Display impl 开始

如果已经有一个普通 impl，直接在它上面加属性也是常用起点。下面是另一个
完整程序：为本地类型 `UserId` 写的 `Display` 实现，同时用于 `OrderId`。
签名来自你写好的 impl，不需要复制外部 trait 的定义。

```rust
use batch_impl::batch_impl;

use std::fmt;

struct UserId(u64);
struct OrderId(u64);

#[batch_impl(@Self: [UserId, OrderId])]
impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

fn main() {
    assert_eq!(UserId(7).to_string(), "7");
    assert_eq!(OrderId(12).to_string(), "12");
}
```

新增的属性中，`@Self` 读取输入 impl 的自身类型 `UserId`，右边列表给出
两个目标。生成结果替代输入块，因此这里把 `UserId` 也列进去，保留它的实现。
`fmt` 签名、字段访问和 `write!` 都是普通 Rust；这些类型具有相同的 `.0` 字段，
所以同一方法体适用。孤儿规则等 Rust 检查仍然照常生效。

本地 trait 定义配 `#方法名{body}`，或已有完整 impl 配 `@Self: 目标列表`，
可按手头已有的代码选择。显式形状模板与多阶段替换属于进一步用法，见
[§8.5](#85-impl-entry080itemimpl-入口)。

### 1.7 维护同一份 Describe

继续修改 [§1.5](#15-转发-box-方法) 的程序，不接前面的 `Display` 示例。
假设现在需要增加 `u64`、让描述支持前缀，并为 `u32` 提供特殊格式：

| 需求 | 手动修改 | 宏继续负责 |
|---|---|---|
| 增加 `u64` | 加入整数目标列表 | 为它生成同样的成员 |
| `describe` 增加 `prefix: &str` | 修改 trait 签名，让各业务 body 使用 `prefix`，为调用补上实参 | 复制新签名；`Box` 委托自动转发新参数 |
| `u32` 使用特殊格式 | 从共享列表移除 `u32`，为它写独立 spec | 各目标分别生成实现，避免同一类型有两份冲突实现 |

复制签名不会自动重写业务逻辑或调用者。下面保留 `kind` 的行为，
只调整需要新格式的 body；原有 `#blanket` 不必修改：

```rust
use batch_impl::batch_impl;

#[batch_impl(
    [u8, u16, u64] #describe{format!("{prefix}number: {self}")} #kind{"integer"},
    u32 #describe{format!("{prefix}wide number: {self}")} #kind{"integer"},
    bool #describe{format!("{prefix}bool: {self}")} #kind{"boolean"},
    <T: std::fmt::Debug> Vec<T> #describe{format!("{prefix}{self:?}")} #kind{"list"},
    #blanket(@all_ref_methods){Box}
)]
trait Describe {
    fn describe(&self, prefix: &str) -> String;
    fn kind(&self) -> &'static str;
}

fn main() {
    assert_eq!(7u8.describe("value: "), "value: number: 7");
    assert_eq!(8u16.describe(""), "number: 8");
    assert_eq!(10u64.describe(""), "number: 10");
    assert_eq!(9u32.describe("value: "), "value: wide number: 9");
    assert_eq!(7u8.kind(), "integer");
    assert_eq!(9u32.kind(), "integer");
    assert_eq!(true.describe(""), "bool: true");
    assert_eq!(true.kind(), "boolean");
    assert_eq!(vec![1u8, 2].describe(""), "[1, 2]");
    assert_eq!(vec![1u8, 2].kind(), "list");
    let boxed = Box::new(vec![1u8, 2]);
    assert_eq!(Describe::describe(&boxed, "value: "), "value: [1, 2]");
    assert_eq!(Describe::kind(&boxed), "list");
    assert_eq!(
        Describe::describe(&Box::new(9u32), "value: "),
        "value: wide number: 9"
    );
    println!("{}", 7u8.describe(""));
}
```

仍使用原有业务逻辑的目标，在空前缀时保持原先格式；`u32` 的新格式也会通过 `Box<u32>` 的委托生效。
这些断言同时覆盖新增目标、修改后的签名、例外实现和参数转发。

### 检查展开

先让程序运行成功；以后修改类型表达式或签名遇到难以判断的错误时，
可以临时用 `batch_impl::batch_preview!` 包住带属性的 trait 查看展开：

```text
batch_impl::batch_preview! {
    #[batch_impl([u8, u16, u32] #describe{format!("number: {self}")})]
    trait Describe { fn describe(&self) -> String; }
}
```

运行 `cargo check`。预览会主动用 `compile_error!` 输出生成的 Rust，因此
这里编译失败是预期行为，不代表 DSL 输入一定有错。检查输出中的目标类型
和方法签名，然后恢复普通属性写法，再运行 `cargo run`。预览只是临时的
检查步骤。

没有方法的标记 trait 只写目标类型即可。更复杂的 impl 还可以在 spec 中
声明泛型、指定 trait 的实参：

```text
<impl-泛型> Trait名<trait-泛型> 目标类型 { body }?
```

| 部分                  | 示例                                    | 何时需要               |
|-----------------------|-----------------------------------------|------------------------|
| `<impl-泛型>`         | `<T>`, `<T: Clone>`, `<const N: usize>` | impl 块需要泛型参数时  |
| `Trait名<trait-泛型>` | `MyTrait<T>`, `MyTrait<Vec<T>>`         | trait 定义有泛型参数时 |
| 目标类型              | `usize`, `Vec<T>`, `&str`               | 必需                   |
| `#方法名{body}`        | `#describe{format!("number: {self}")}` | 复制该方法的签名并提供方法体 |
| `{ body }`            | `{ fn m(&self) -> usize { 0 } }`        | 需要自定义实现体时     |

多个 spec 用 `,` 分隔：`#[batch_impl(usize, isize)]`。

**规模上能省多少。** [simplify.rs](https://github.com/5-6-1/batch-impl-rs/blob/main/examples/simplify.rs) 用大约 **15 行** DSL 得到 **30 个 impl**（手写约 80 行），[typeclass.rs](https://github.com/5-6-1/batch-impl-rs/blob/main/examples/typeclass.rs) 覆盖一个类层级外加 36 个 `From<bool>` 实例。两者都由 CI 编译，准备好阅读更大示例时，可以跟随 §13 的讲解。

## 2. 类型矩阵：空格（与 `.`）

**空格是主推的写法**：容器/修饰符与它接收的类型并排写——链式累加参数（左结合）。

`Box u32` 给 `Box` 一个实参，`HashMap u32 String` 给 `HashMap` 两个
实参。需要让实参本身成为组合类型时，用括号分组：`Box (Vec u32)`
得到 `Box<Vec<u32>>`。

| 写法                   | 展开                                  |
|------------------------|---------------------------------------|
| `Box u32`              | `Box<u32>`                            |
| `Box (Vec u32)`        | `Box<Vec<u32>>`（内层类型分组）       |
| `HashMap u32 String`   | `HashMap<u32, String>`（左结合累加）  |
| `fn(A,B) C`            | `fn(A,B)->C`（也可写 `fn(A,B) -> C`） |
| `&u8`                  | `&u8`（修饰符链式应用）               |
| `Tr u8`                | `impl Tr for u8`（裸 trait 名）       |
| `[Box, Vec] u32`       | `Box<u32>, Vec<u32>`（列表展开）      |
| `HashMap<u8> String`   | `HashMap<u8, String>`（预填泛型追加） |
| `Box [u8, u16]`        | `Box<u8>, Box<u16>`（列表分发）       |
| `[Box, Vec] [u8, u16]` | 笛卡尔积共 4 项                       |

**`.` 是同一运算的右结合形态。** 它提供另一种嵌套写法：`Box.Vec.u32`
和 `Box (Vec u32)` 都得到 `Box<Vec<u32>>`。没有分组时，空格累加实参：
`Box Vec u32` 得到 `Box<Vec, u32>`。

| 写法                     | 展开                                 |
|--------------------------|--------------------------------------|
| `Box.Box.u8`             | `Box<Box<u8>>`（右结合嵌套）         |
| `Box.Vec.u32`            | `Box<Vec<u32>>`（同 `Box (Vec u32)`） |
| `&Box u8`                | `&Box<u8>`（修饰符作用于嵌套类型）   |
| `[Box, Vec] T`           | `Box<T>, Vec<T>`                     |
| `Box [T1, T2]`           | `Box<T1>, Box<T2>`                   |
| `[HashMap<K>, Vec<K>] V` | `HashMap<K, V>, Vec<K, V>`           |

> **什么时候用哪个**：容器与它的实参并排写（`Box u8`、`HashMap<u8> String`）。
> 嵌套容器时，用括号分组内层类型（`Box (Vec u32)`），也可以用点号
> （`Box.Vec.u32`）。

优先级从低到高：`;` < `,` < 空格 < `.`，`()` 分组在所有运算符之上。

**被注解 trait 自己的名字**才按 impl trait 应用：属性挂在 `trait Tr` 上时，写 `Tr u8` 得到 `impl Tr for u8`、`Tr<A> u8` 得到 `impl Tr<A> for u8`。**其他**裸标识符是类型而不是 trait 头——属性挂在 `Tr` 上时，spec `Other u8` 产出 `impl Tr for Other<u8>`。要**类型** `Tr<u8>` 直接写 `Tr<u8>`。一般情况下，不推荐使用裸 `Tr`。

> **混合分组**：`Box.Vec u32` 中点号先分组，随后空格再添加一个实参，
> 因此得到 `Box<Vec, u32>`。如果想要 `Box<Vec<u32>>`，应写
> `Box (Vec u32)` 或 `Box.Vec.u32`。

> **操作数严格性**：`.`/`,` 两侧必须有操作数——`A.`、`.A`、`,A`、`A,,B` 均报 `compile_error!`；仅**尾随逗号**（`A,` / `[A, B,]`）允许，`()`/`[]` 等括号是真实 token 不算空操作数。`;` 作为 `batch_trait!` 段落边界保持宽松。

```rust
use batch_impl::batch_impl;
use std::collections::HashMap;
#[batch_impl(Box (Vec u32), HashMap<u8> String)]
trait T {}
// → impl T for Box<Vec<u32>> {}   （也可写 Box.Vec.u32）
// → impl T for HashMap<u8, String> {}
```

## 3. 列表与 body

### 并列列表 `[A, B]`

一个 body 为所有目标类型复用。这是 §1.1 中 `#describe` 的另一种写法：
在共享 body 里写完整方法，得到相同的三个实现。

```rust
use batch_impl::batch_impl;
#[batch_impl([u8, u16, u32] {
    fn describe(&self) -> String { format!("number: {self}") }
})]
trait Describe { fn describe(&self) -> String; }
// → impl Describe for u8 { fn describe(&self) -> String { format!("number: {self}") } }
// → impl Describe for u16 { ... }
// → impl Describe for u32 { ... }
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

元组或泛型实参中的 `[A, B]` 会分发到各个目标；多个列表产生所有组合。
嵌套列表同样会展开：`Vec<[[A,B], C]>` 生成 `Vec<A>`、`Vec<B>`、`Vec<C>`。
列表缩到一个类型时仍写 `[A,]` 或直接写 `A`；`[A]` 是切片类型。

### 独立/共享 body 合并

列表项可有独立 body，与共享 body 合并。合并会把成员放进同一个 impl，
不按名字覆盖；同一成员出现两次会由 Rust 报错。下面的独立 `name` 和共享
`zero` 是不同成员，因此可以组合：

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
// → impl Zero for isize { fn zero() -> Self { Default::default() } fn name(&self) -> &'static str { "isize" } }
```

要让某个类型使用不同的同名方法，像
[§1.3](#13-特殊类型用独立-spec)
那样拆成独立 spec，使每个目标只得到一份方法定义。

## 4. 包 `*`——映射与拼入

包是一组等待宿主接收的类型表达式。`*X` 取出**候选列表**的直接成员；
其他一切类型——元组、unit 类型、切片、数组——都成为单成员包。
星号正是用来区分 `*[A, B]`（两个成员）与 `*(A, B)`（一个成员，其类型即该元组），
以及 `*[]`（空包）与 `*()`（一个成员：unit 类型）的。

常见用法只需三步：**打开成员、应用规则、放入结果**。
`(*Vec *[].3,)` 打开三个独立参数，分别套上 `Vec`，最后放入一个元组。

### 4.1 列表 / 元组内拼入

包里的普通类型保持完整。嵌套包可以拼入，但普通元组仍是一个类型。

```rust
use batch_impl::batch_impl;

#[batch_impl([u8, *[u16, u32]])]
trait Each {}

#[batch_impl((u8, *[u16, u32]))]
trait Together {}

#[batch_impl(*(u8, u16))]
trait OneTuple {}

fn main() {
    fn each<T: Each>() {}
    fn together<T: Together>() {}
    fn one<T: OneTuple>() {}
    each::<u32>();
    together::<(u8, u16, u32)>();
    one::<(u8, u16)>();
}
```

### 4.2 左操作数：对每个成员应用同一规则

左包把其中每个成员应用于右侧。`*[Vec, Box] u8` 得到 `Vec<u8>`、`Box<u8>`。
普通左类型则把右包放进一个实参槽：
`Pair *[u8, u16]` 在消费该槽时成为 `Pair<u8, u16>`。

两侧都是包时，每个**右侧直接成员是一行**。所有左成员都接收这一整行。
右行在外，左成员在内；一次映射任务不会重新打开已经选中的行。

```rust
use batch_impl::batch_impl;

struct Pair<A, B>(A, B);

#[batch_impl((*Vec *[].1..=3,))]
trait Wrapped {}

#[batch_impl((*Pair (*[self, Vec] *[].1..=3),))]
trait Paired {}

#[batch_impl((*() (*[self, Vec] *[].3),))]
trait Rows {}

fn main() {
    fn wrapped<T: Wrapped>() {}
    fn paired<T: Paired>() {}
    fn rows<T: Rows>() {}
    wrapped::<(Vec<u8>, Vec<bool>)>();
    paired::<(Pair<u8, Vec<u8>>, Pair<bool, Vec<bool>>)>();
    rows::<((u8, Vec<u8>), (bool, Vec<bool>), (i32, Vec<i32>))>();
}
```

`self` 返回整个实参。因此 `*[self, Vec]` 对每个独立生成的 `T`
构造 `T, Vec<T>` 两个成员。`*Pair` 把每行收进泛型实参，
`*()` 把每行收进元组元素。这两个例子使用同一条映射规则。

空格仍然左结合。要得到 `(Vec<Box<T0>>, Vec<Box<T1>>)`，
写 `(*Vec (*Box *[].2),)`；`*Vec *Box *[].2` 会先构造
`Vec<Box>`，再追加一个实参。

### 4.3 泛型实参与 trait 路径

字面量尖括号消费其中的实参槽，不会重新执行应用。
普通类型实参与 trait 实参都可以拼入包。

```rust
use batch_impl::batch_impl;

struct Pair<A, B>(A, B);

#[batch_impl(Pair<*[u8, u16]>)]
trait Concrete {}

#[batch_impl(Convert<*[u8, u16]> Pair<u8, u16>)]
trait Convert<A, B> {}

#[batch_impl(Pair<*[u8, u16].2>)]
trait Matrix {}

fn main() {
    fn concrete<T: Concrete>() {}
    fn convert<T: Convert<u8, u16>>() {}
    fn matrix<T: Matrix>() {}
    concrete::<Pair<u8, u16>>();
    convert::<Pair<u8, u16>>();
    matrix::<Pair<u8, u16>>();
    matrix::<Pair<u16, u8>>();
}
```

最后一个表达式先选择两个位置的笛卡尔积，再填入两个泛型实参槽。
普通候选列表仍然分支：
`Pair<*[u8, [u16, u32]]>` 得到两条 impl，而非三个实参。

### 4.4 容器规则

括号不检查内部是什么类型：`(X)` 是分组，`(X,)` 是元组。
同样，`[X]` 是切片，`[X,]` 是候选列表。
因此 `(*[u8, u16])` 是加了分组的包（两条目标 impl），
`(*[u8, u16],)` 才是一个元组。
`[*[u8, u16]]` 不合法：切片只有一个元素类型槽。

**列表会分发**，而包在这里被拒绝：切片/数组的元素槽收下列表，
所以 `[[u8, u16]]` 得到两个切片，`[[u8, u16]; 4]` 得到 `[u8; 4]` 与 `[u16; 4]`——
没有构造器拼写时，这就是把家族包进切片/数组的方式。同一槽位里放包则是上面的基数错误。

嵌套前缀幂等：`*(*X)` 就是 `*X`，没有额外的双星操作。
要保留一行，将它收进普通元组或泛型宿主，如 §4.2 所示。

### 4.5 生成器与维度

`*[].N` 生成含 `N` 个独立参数的包。复制已生成的成员保留参数身份；
执行另一个生成器才创建另一组。`*[].0` 不产生参数。

空是**基座**，不是目标：裸写的 `*[]` 是展开为零的 spec，会给出诊断
（把目标写出来，或给它长度）；而 `*[].N` 才是上面的生成器。
实参位置里的空包（`Vec<*[]>`）同样给诊断。

普通元组的幂仍然复制其**直接槽**：
`([u8, u16],).2` 有四种组合，
`(*[u8, u16],).2` 则物化为 `(u8, u16, u8, u16)`。
包的幂先拼平嵌套包，再将所得成员作为候选。

两个轴仍使用同一条应用规则。保留行可以让维度体现在 Rust 类型中：

```rust
use batch_impl::batch_impl;

struct Map<T, U>(T, U);

#[batch_impl((*() (*Map *[].1..=2 *[].1..=3),))]
trait Grid {}

fn main() {
    fn grid<T: Grid>() {}
    grid::<(
        (Map<u8, bool>, Map<u16, bool>),
        (Map<u8, i32>, Map<u16, i32>),
        (Map<u8, char>, Map<u16, char>),
    )>();
}
```

共有六种形状。固定维度 `(*Map *[].2 *[].3,)` 则把六个成员拼成平坦元组，
共享五个参数。两个维度都使用*范围*时，拼平结果可能产生重叠 impl：
`1 × 2` 与 `2 × 1` 的模式可能描述同一个 Rust 类型。
宏保留两者，由 rustc 报 E0119；未使用的泛型声明也不会自动删去，
第二轴为空时可能留下 E0207。
更多推导见包模型的[应用教程](https://github.com/5-6-1/batch-impl-rs/blob/main/tests/pack_model/tutorial.zh-CN.md)。

### 4.6 合法位置

元组元素、泛型与 trait 实参、callable 参数接受多个成员。
引用与指针目标、切片/数组的元素类型、函数返回值、单个 bound 和关联类型绑定值
要求**每个分支恰好一个类型**，空包或多成员包在这些位置得到定向错误。

声明块拼入名字（`<*[A, B]>`）；fresh 生成器不能在此声明名字，
因为它自己携带的声明没有目标可以承载。
构造类型也不是参数声明，例如 `<*[Vec<u8>,]>` 会报错。
原始指针 `*const T`、`*mut T` 保留 Rust 意义，没有后续块的裸 `*` 报错。

`where{...}` 谓词和 `impl{...}` 形状模板保持标准 Rust 类型语法域，
仅保留原有的 `@` 替换；body 和指令参数仍由各自语法解释，不引入包运算。
限定路径的 `::Assoc<...>` 续接部分与 `<T as Trait>` 中 `as` 后的 trait 路径
保持普通 Rust 路径，不在内部拼入包。
见[参考手册](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/reference.md) §2 与 §4。

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
// → impl<T: Clone, const N: usize> A<T, N> for Vec<u8> where T : Clone {}
```

这个简写属于 **spec 头部**（trait 应用处）：正是它在声明那些形参。写在别处——
例如目标类型上——空 `<>` 就是 §6.4 的**同步标记**，会被填上本 spec 的实参
（`Swap2<>` → `Swap2<T>`），绝不会在 spec 中间吐出一个声明块。

### 5.3 实参：多实参、嵌套、绑定

```rust
# use batch_impl::batch_impl;
struct Map<K, V>(K, V);
struct A; struct B; struct C;
struct Wrap<X>(X);
#[batch_impl(Map<A, B>)]                 // 多实参
trait M1 {}
#[batch_impl(Map<Map<A, B>, C>)]         // 嵌套类型作为一个实参保留
trait M2 {}
#[batch_impl(Conv<u8, Item = u8> Wrap<u8>)]  // 关联类型绑定（trait 路径）
trait Conv<T> { type Item; }
```

### 5.4 `<>` 内的操作（0.7.0 可编程化）

> **按需扩展。** 普通泛型只需声明与使用参数。下面是在实参内部生成类型
> 组合的写法；暂时不需要生成器时，可以直接继续 §5.5。

泛型实参位置可以写完整的 DSL 表达式——这是 0.7.0 的结构化落地：

```rust
# use batch_impl::batch_impl;
struct Wrap<X>(X);
struct Pair3<A, B>(A, B);
struct A2; struct B2;

#[batch_impl(Wrap<()2>)]               // generator：<P0,P1> Wrap<(P0,P1)>
trait GenTup {}
// → impl<P0,P1> GenTup for Wrap<(P0, P1)>（元组保持单个实参）

#[batch_impl(Pair3<*[].2>)]             // generator splat：<P0,P1> Pair3<P0,P1>
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

**目标以 `::` 开头时必须显式写出元素边界**：`<...>`（跟在 ident 之后）与 `::` 都是**当前路径的续接**，而空格与 `.` 是元素边界，所以并列写法会把头和绝对路径目标粘成一条路径——trait 落进类型位置（E0782）。把 `.` 写出来：

```rust
# use batch_impl::batch_impl;
// `@trait<u8> . ::std::string::String` → impl TrE<u8> for ::std::string::String
#[batch_impl(@trait<u8> . ::std::string::String)]
trait TrE<T = usize> { fn tag(&self) -> u8 { 7 } }
```

有 trait 头时空格与 `.` 等价；edition 2024 里 `::name` 指**外部 crate**（本 crate 根写 `crate::...`）。完整规则与它的边界表——粘连形态、组是实参追加、单元素 spec 整体是目标——见 `docs/zh-CN/reference.md` §1.2。

## 6. `@` 常量系统（宏元层）

`@` 是 DSL 预留的**库专属常量命名空间**——`#` 被指令机制占用，`@` 提供"命名并复用类型矩阵条目"的能力。它是纯**词法替换**（宏元层）：展开结果进入后续管道，不参与任何域内解析。

> **按需阅读。** §6 讲常量与生成参数的寻址。§1 用过的基础签名复制指令
> 在 §7.1–§7.2 详述；§7.3–§7.5 的委托和扩展可以以后再读。普通约束、
> 元组和修饰符则继续看 §8.1–§8.3、§9 和 §10。

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

**范围族**选择一族内的连续段。与 Rust 一致，`..` 排除写出的上端点，`..=` 包含它：`@u8..u16` → `u8`，`@u8..=u16` → `u8, u16`。`@u8..=u128`、`@i8..=i128`、`@f32..=f64` 选择各自完整的位宽族。

任一端点可以**省略**：`@..u16` ≡ `@u8..u16`，`@..=u16` ≡ `@u8..=u16`，`@u16..` ≡ `@u16..=u128`，`@f32..` ≡ `@f32..=f64`。至少一个端点用于确定类型族；`..=` 必须写上端点。`usize`/`isize` 只进名字族，不进范围族。

反向范围以及 `@u8..u8`、`@..u8` 这样的空排他范围报错，不会静默生成空类型矩阵。

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
| `@N..=M`     | 连续段                                 | fresh N..=M，逗号分隔（`@0..=1` → `P0, P1`）                                               |
| `@N..`       | 到最后一个 fresh 的**开放**段          | 从 N 到最后一个 fresh，逗号分隔（`@1..` → `P1, P2, ...`）；N 越界时**为空**（arity 1 的 impl 不产生此类谓词，不报错） |

fresh 的显示名按文档序编号为 `P0, P1, ...`（与 impl 已用 ident 冲突时按表格字母序后缀逃逸：`P0A`、`P0B`、…、`P0Z`、`P0AA`）——展开把名字拼到 `@` 所在位置（where 谓词主体、target 元组元素、泛型实参），范围变成多个名字，`where` 尾部逐 fresh 复制。

> **高级用法**：`@g_i` / `@N..M` 是高级寻址记号——日常从 `@u*` / `@all_methods` / `@0` 起步，只有谓词必须指名某个特定 fresh 时才动用。兼容性约定见 [README](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/README.md)，明确的迁移说明见 [CHANGELOG](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/CHANGELOG.md)。

> **引用里的 `_` 是"组/位"分隔符**，不是 Rust 的数字分隔符：`@1_0` 是组 1、位 0。想要扁平下标就写 `@1000`（不带分隔符）；`@1_000` 的读法见参考手册 §5 的边界材料。

```rust
# use batch_impl::batch_impl;
#[batch_impl(()2 where @0..=1: Clone)]   // 范围糖：@0..=1 = @0, @1
trait RangeSugar {}
// → impl<P0,P1> RangeSugar for (P0,P1) where P0: Clone, P1: Clone

#[batch_impl(()3 where @0..: Copy)]       // from 0 to the last fresh
trait AllFresh {}
// → impl<P0,P1,P2> AllFresh for (P0,P1,P2) where P0: Copy, P1: Copy, P2: Copy

#[batch_impl(()3 where @1..: Copy)]       // 开放范围：从下标 1 起
trait OpenRange {}
// → impl<P0,P1,P2> OpenRange for (P0,P1,P2) where P1: Copy, P2: Copy
// （arity 1 的 impl 不产生任何谓词——那里 `@1..` 为空）
```

**`@all_fresh` 已移除**：将既有用法改为 `@0..`。
`@N..` 家族中，`@0..` 覆盖整段，`@1..` 覆盖尾部。

**范围在单个 `@N` 能出现的任何位置都可用**（0.9.2）：除了上面的 where
谓词，范围的尾部也可以是关联类型路径，逐 fresh 复制——目标位置的
范围对照本 spec 生成器产出的 fresh 列表重新展开：

```rust
# use batch_impl::batch_impl;
struct Wrap3<A, B, C>(A, B, C);
#[batch_impl(Wrap3<*[].3> where @0..: Clone { fn m(&self) {} })]
trait RangeAngle { fn m(&self); }
// → impl<P0,P1,P2> RangeAngle for Wrap3<P0,P1,P2> where P0: Clone, P1: Clone, P2: Clone

trait HasOut { type Out; }
#[batch_impl(Wrap3<*[].3> where @0..: HasOut, @0..::Out: Clone { fn m(&self) {} })]
trait RangeAssoc { fn m(&self); }
// → where P0: HasOut, P1: HasOut, P2: HasOut, P0::Out: Clone, P1::Out: Clone, P2::Out: Clone
```

范围索引的 fresh 列表来自本 spec 的生成器（`*[].N` / `().N`）；spec 无
fresh 泛型时范围报 "out of range"。

**impl 泛型声明位置同样可用**：`<@0..>` 把范围覆盖的每个 fresh 声明为
impl 参数——生成器放在 trait 实参（`GenConv<*[].2>`），声明与谓词引用
同一批 fresh：

```rust
# use batch_impl::batch_impl;
struct DeclTarget;
#[batch_impl(<@0..> GenConv<*[].2> DeclTarget where @0..: Clone { fn m(&self) {} })]
trait GenConv<T, U> { fn m(&self); }
// → impl<P0,P1> GenConv<P0,P1> for DeclTarget where P0: Clone, P1: Clone
```

（空的 `<@0..>`——spec 无 fresh 生成器——不产生任何参数，如同空的
`@1..` 谓词。）

**组内范围 `@L_N..`**（0.9.2）在**单个生成器组内**切片——`@g_i` 的组内对应物，
跨数组分发稳定。一个 spec 里有多个生成器时（如 `PairGen<*[].2, *[].3>`），
第一个是组 0、第二个是组 1；`@1_0..` 只约束组 1 的 fresh：

```rust
# use batch_impl::batch_impl;
struct MultiTarget;
#[batch_impl(
    <@0..> <@1..> PairGen<*[].2, *[].3> MultiTarget where @1_0..: Clone
    { fn m(&self) {} }
)]
trait PairGen<A, B, C, D, E> { fn m(&self); }
// → impl<P0,P1,P2,P3,P4> PairGen<P0,P1,P2,P3,P4> for MultiTarget
//     where P2: Clone, P3: Clone, P4: Clone   ← 仅组 1（P0,P1 无约束）
```

`@L_N..`（到组尾开放）、`@L_N..M`（不含 M）、`@L_N..=M`（包含 M）都可用；未知组报错
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
| `@Self` | **输入类型**——当前 impl 属性收到的自身类型 | 将原型复用为模板或类型实参（§8.5） |
| `@all_methods` 等 | **选择**——从 trait_def 提取 item 集合            | `#fill(@all_required_methods, -foo)` 精确选中 |
| `@Cow`            | **`#blanket` 专属内置包装常量**——`Cow<'_>` 及其打包的约束（`@0: ToOwned + ?Sized, @0::Owned: @trait`） | blanket 可用的 `Cow` 委托（见 §7.4）          |

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

它把标记填在类型所在的任何位置，`@trait<>` 等价（`@trait` 先展开为 trait 路径）。写它时要知道两件事：该标记**不看名字**——填进去的是本 spec 的实参，所以 `Other<>` 会变成 `Other<…spec 实参…>`，那里的元数不匹配由 rustc 报；**body 内部**只有通过**开关模板** `impl{Tr<>}` 才同步（body 是任意 Rust，`Vec<>` 不是 trait 引用）。全部会同步的表面在 `docs/zh-CN/reference.md` §13.2。

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
// → impl<R, T: Fn(P0) -> R, P0>  MultiArity<T, R> for (P0,)
// → impl<R, T: Fn(P0,P1)->R, P0,P1> MultiArity<T, R> for (P0,P1)
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
#[batch_impl((u32,) #fill([add, add2]){self.0 = self.0.wrapping_add(x as u32)})]
trait Ops { fn add(&mut self, x: u8); fn add2(&mut self, x: u8); }
```

参数可以是名字列表、`@all` 系 marker，配合 `-name` 排除：

```rust
# use batch_impl::batch_impl;
#[batch_impl(u8 #fill(@all_methods, -extra){ 0 })]
trait Markers {
    fn marker(&self) -> u8;
    fn extra(&self) -> u8 {
        7
    }
}
// → impl Markers for u8 { fn marker(&self) -> u8 { 0 } }（被排除的 `extra` 保留默认实现）
```

> 只填一个方法时，`#fill([foo]){body}` 与单 item 指令 `#foo{body}` 等价，后者更简洁。

共享的作用域语法接受尾逗号（`#fill([marker,],){0}`）以及空选择：`#fill(){...}`、`#fill([]){...}`、结果为空的 `@all` 家族或删去全部成员的差集。此时 `#fill`、`#delegate` 不生成成员；`#blanket` 仍生成包装 impl。未实现的必需成员由 Rust 检查。所有显式写出的 trait 成员名都必须存在，包括被排除的名字；`#fill(typo, -typo){...}` 不能靠差集隐藏拼写错误。

### 7.3 `#delegate(methods){...}` — 委托调用

```rust
# use batch_impl::batch_impl;
#[batch_impl(
    Vec<u32> #d_len{self.len()},
    Box.Vec.u32 #delegate(d_len){**self}
)]
trait MyLen { fn d_len(&self) -> usize; }
// → impl MyLen for Box<Vec<u32>> { fn d_len(&self) -> usize { (**self).d_len() } }
```

分支内的委托写作 `receiver.#call`。它是当前方法的一次完整调用，自动转发
参数，无需追加 `()`。出现可识别的标记时，整个指令内容就是方法体；没有
标记时仍是上例的目标表达式。不需要额外的模板标记。

```rust
# use batch_impl::batch_impl;
enum Buffer { Text(String), Bytes(Vec<u8>) }

#[batch_impl(Buffer #delegate(@all_methods, size = len){
    match self {
        Self::Text(inner) => inner.#call,
        Self::Bytes(inner) => inner.#call,
    }
})]
trait BufferOps {
    fn size(&self) -> usize;
    fn truncate(&mut self, len: usize);
}

let mut text = Buffer::Text("abcd".into());
let mut bytes = Buffer::Bytes(vec![1, 2, 3, 4]);
text.truncate(2);
bytes.truncate(3);
assert_eq!(text.size(), 2);
assert_eq!(bytes.size(), 3);
```

两种接收者留在各自分支中，不需要统一类型。`size = len` 使 `size` 中的
调用变成 `inner.len()`，`truncate` 中则是 `inner.truncate(len)`。
接收者也可以是 `self.inner.as_ref().#call` 这样的表达式；
`inner.#call.into()` 从调用结果继续链式操作。方法的类型/const 泛型
显式转发，生命周期保持推断。不会自动添加 `.await`，需要时写
`inner.#call.await`。
继续追加 `()` 是调用返回值本身，因此 `inner.#call()` 要求返回值可调用。

标记仅在该 delegate body 的表达式中识别。宏 token、属性和内嵌 item
定义不改写，也不会触发方法体形式。普通 `.call(...)` 方法和开放扩展
指令名保持原义。完整规则见参考手册 §6.5。

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
#[batch_impl(#blanket(@all_methods){Box})]
trait NumOps { fn inc(&mut self); }
impl NumOps for u32 { fn inc(&mut self) { *self += 1 } }
// → impl<P0> NumOps for Box<P0> where P0: NumOps { fn inc(&mut self) { <_ as NumOps>::inc(&mut **self) } }
//   （面向所有满足 P0: NumOps 的类型，不限于 u32）

#[batch_impl(#blanket(@all_methods){&, Box})]
trait Len { fn len(&self) -> usize; }
// → impl<P0> Len for &P0     where P0: Len { fn len(&self) -> usize { <_ as Len>::len(&**self) } }
// → impl<P0> Len for Box<P0> where P0: Len { fn len(&self) -> usize { <_ as Len>::len(&**self) } }
```

调用显式限定当前 trait，因此 supertrait 的同名方法不会造成委托歧义。接收者类型由解引用后的实际值推断；静态方法使用 `<T as Trait>::method(...)`。异步方法追加 `.await`，方法的类型/const 泛型显式传递，例如 `<_ as Trait>::read::<U, N>(&**self, value).await`；生命周期继续推断。

> **`:N` deref 深度**——要穿过多少层包装。单层包装（`&`、`Box`、`Rc`）默认 **1**：引用接收者需要 N+1 次解引用再显式借用（`&**self` 或 `&mut **self`）。`Box.Arc:2` = `Box<Arc<T>>`，共享借用调用如 `<_ as Trait>::method(&***self)`。任意包装的 deref 目标不一定就是它的类型参数。单层包装不用写深度后缀。

> **按值接收者**：`fn consume(self)` 转发为 `<_ as Trait>::consume(*self)`——按值 `self` 本身就是包装，少一层 deref。包装必须允许移出该值，或者内部值满足 `Copy`；生成物附带 `#[doc]` 提示。跳过这类方法用 `@all_ref_methods`（保留 trait 默认），或手写 `#name{...}`。显式 `self: &Self` / `self: &mut Self` 使用对应的引用规则。

#### GAT、`Self` 与非 Sized 目标（0.9.4）

**泛型关联类型（GAT）** 用带自身参数名的投影委托——`trait Iterable { type Iter<'a>: Clone where Self: 'a; }` 变成 `type Iter<'a> = <T as Iterable>::Iter<'a> where Self: 'a;`（裸投影缺生命周期实参，E0107）。普通关联类型/常量保持既有 `<T as Trait>::Item` 投影。`: Clone` 这类结果约束保留在 trait 声明上；impl 定义保留 GAT 参数声明与 `where` 谓词，投影只传生命周期、类型与 const 参数的名字。`#fill`、`#name` 使用同一条关联类型声明规则。

**裸 `Self`** 出现在方法的普通参数、返回值或泛型约束里无法 blanket 委托，因为包装和内部类型不同。这包括 `U: Marker<Self>`、`where U: Marker<Self>` 及 `where Self: Marker<U>`；报错会建议手写 `#name{...}`。接收者里的 `Self`、`where Self: Sized` 及 `Self: 'a` / `Self: Sized + 'a` 这类 outlives 条件仍可使用，实际委托目标是否满足条件由 Rust 检查。`Self::Assoc` 在参数、返回值及约束中都允许：转发关联项后，两边投影为同一类型。属性负载不会被当作约束读取。

**`@?` 非 Sized 后缀**：以 `@?` 结尾的包装（`Box@?`）给该 spec 的 where 子句加 `T: ?Sized`，fresh 泛型可以是非 Sized 目标：

```rust
# use batch_impl::batch_impl;
#[batch_impl(#blanket(@all_methods){Box@?})]
trait DynLen { fn dlen(&self) -> usize; }
impl DynLen for str { fn dlen(&self) -> usize { self.len() } }
// → impl<P0> DynLen for Box<P0> where P0: DynLen, P0: ?Sized——fresh（以及目标）可以是非 Sized
```

#### `@Cow`——携带约束的打包（示范案例）

`@Cow` 是 **`#blanket` 专属内置包装常量**（只在 `#blanket` 包装列表里可用），把 `Cow<'_>` 与 `@0: ToOwned + ?Sized`、`@0::Owned: @trait` 一起打包。`Cow<'_, T>` 解引用得到 `T`；`T::Owned: Trait` 是该常量额外附带的约束。包装与打包的谓词一起进入普通 blanket 管线：

```rust
# use batch_impl::batch_impl;
# use std::borrow::Cow;
#[batch_impl(#blanket(@all_methods){@Cow})]
trait CowLen { fn clen(&self) -> usize; }
impl CowLen for str { fn clen(&self) -> usize { self.len() } }
impl CowLen for String { fn clen(&self) -> usize { self.len() } }
// → impl<P0> CowLen for Cow<'_, P0> where P0: CowLen, P0: ToOwned + ?Sized, P0::Owned: CowLen
//   （一个泛型 impl 覆盖满足这些打包约束的目标）
```

### 7.5 开放扩展（顶层宏注入）

名字不是 `fill`、`delegate` 或 `blanket` 的 `#name(args){body}` 会调用同名
函数式宏，即使 trait 中也有同名成员；不带 `(args)` 的 `#name{body}` 才是
成员赋值。下面使用库提供的开放扩展参考宏 `batch_preprocess_test!`，
让它为两个方法生成实现；这不是只预处理或显示 token 的工具。

```rust
use batch_impl::{batch_impl, batch_preprocess_test};

#[batch_impl(u16 #batch_preprocess_test(add,inc){*self + 3})]
trait AddIncU16 {
    fn add(&self) -> Self;
    fn inc(&self) -> Self;
}

fn main() {
    assert_eq!(5u16.add(), 8);
    assert_eq!(5u16.inc(), 8);
}
```

batch-impl 将目标、所选方法名、body 和 trait 定义按
`{spec}(args){body} trait_def` 四段协议交给该宏；参考宏从 trait 复制签名，
使用 `*self + 3` 作为返回值，生成完整 impl。编写自己的扩展时，实际如何
生成代码由你的宏决定。手动四段写法见
[开放扩展 API 文档（仓库源码）](https://github.com/5-6-1/batch-impl-rs/blob/main/src/doc/directive_open.md)。

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

约束与代码块分离的 Rust 风格写法（有 body 时，它的 `{...}` 跟在谓词后面）：

> 等价地，`where{谓词} {代码块}`（§8.1 后缀 + 链式 body）也可以裸写成 `where 谓词 {代码块}`，省一层 `{}`。

```rust
# use batch_impl::batch_impl;
#[batch_impl(u8 where u8: Clone { fn tag(&self) -> &'static str { "u8" } })]
trait T { fn tag(&self) -> &'static str; }
```

### 8.3 谓词继承

trait 级 where 谓词按**位置替换**并入 impl：改名的参数照样保留它的谓词，谓词文本跟随同一位置上的实参（§5.5、参考手册 §7.2）。谓词指到 impl 没声明的东西时逐字通过，因此由 rustc 报未知类型。谓词里允许什么——`@N` 引用、`Trait<>` 填充、终检会拒掉什么——见 `docs/zh-CN/reference.md` §7。

### 8.4 `impl{...}` shape template 形状模板（0.8.0）

> **进阶层——可跳过。** §8.1–§8.3 讲普通 `where` 约束；本节进一步讲形状模板。
> 只需批量化一个普通 impl 时，§1.6 已经给出了常用入口；需要按类型的组成部分
> 替换实现时，再读这里的形状匹配规则。

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

模板块内是**标准 Rust 类型**——DSL 算子被拒绝，`_` 是通配（匹配任何东西），数组长度可以绑定 const 参数（`impl{[A; N]}` 绑定 `N := 3`，body 里可用）。函数指针的参数与返回类型递归匹配；trait object 模板仍逐字比较。泛型实参里，已声明的 const 名可以绑定 const 值（声明 `const N: usize` 后，`Wrap<N>` 可匹配 `Wrap<3>`）；由声明区分它与类型名，实际的类型/生命周期/const 种类仍不跨类绑定。完整表格（含保留形态 `[A; ()]`）见 `docs/zh-CN/reference.md` §8.2。

#### 原型实现模式

为**代表叶子**写一个正确实现，"相同→保留、不同→绑定"规则会自动适配矩阵中的每个叶子：

```rust
# use batch_impl::batch_impl;
# use std::rc::Rc;
#[batch_impl([Box, Rc] @num impl{Box<u8>} #max{Box::new(u8::MAX)})]
trait TMax { fn max() -> Self; }
// → impl TMax for Box<u8>  { fn max() -> Self  { Box::new(u8::MAX) } }
// → impl TMax for Box<u16> { fn max() -> Self { Box::new(u16::MAX) } }
// → impl TMax for Rc<f64>  { fn max() -> Self  { Rc::new(f64::MAX) } }
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

alga2 风格端到端——一条 spec 覆盖所有元组 arity，`@0..` 给每个 fresh 泛型加约束：

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

> **入口的进一步用法。** 普通 impl 的常用写法见
> [§1.6](#16-从普通-display-impl-开始)。
> 本节补充显式模板、继承规则与多阶段替换，不是使用该入口的前置要求。

**整个 impl 块成为原型。** 把一个普通 Rust impl 交给 `#[batch_impl]`，
写 `@Self: 矩阵`。常量将输入的自身类型复制为模板；每个矩阵叶子与该模板
匹配，所得替换作用到自身类型、trait 实参、where 谓词与 body。
生成的 impl 会替代输入块：

```rust
# use batch_impl::batch_impl;
# use std::rc::Rc;
# trait Make { fn make() -> Self; }
#[batch_impl(@Self: [Box, Rc] [usize, isize])]
impl Make for Box<u8> { fn make() -> Self { Box::new(u8::default()) } }
// → impl Make for Box<usize> { fn make() -> Self { Box::new(usize::default()) } }
// → ... × 4
# assert_eq!(*<Box<usize> as Make>::make(), 0);
# assert_eq!(*<Rc<isize> as Make>::make(), 0);
```

显式模板（`A<B>: 矩阵` 或 `(A, B): 配对列表`）仍可独立于自身类型，描述
整个块内的位置。`@Self` 是输入类型，普通 Rust `Self` 保持 Rust 原义。
堆叠属性各自读取本层输入；后续形状映射将复制的类型视为用户手写的同一段
token（参考手册 §9.2）。

本入口接受 `模板 : 矩阵`（`A<B> : [Box,Rc] [usize,isize]`）或直接形态（`<T> Box<T>`），用 `;` 分隔多个 spec，允许 `@trait` 出现在泛型声明 bound 与 `where` 谓词里（自定义 `@` 常量与 `#` 指令在这里被拒），并保留块自己的泛型、`where` 子句与 `unsafe`。**空** spec 列表是无操作：属性只从该块**派生** impl，没有可派生内容时原块原样回来。规则在 `docs/zh-CN/reference.md` §9.2–§9.3。

**属性堆叠是同一次派生的多个 stage。** 块上方的第二个（第三个……）`#[batch_impl]`
不是又一份 spec 列表：rustc 先展开最外层，本入口把其余属性重新发射到它派生的 impl 上，
下一阶段再**在那些 impl 上**展开——于是各步按源码顺序作用于累积中的块：前一步留在原地的
槽位由后一步绑定，空 stage 就是恒等。写在两步之间的普通属性属于它所在的展开层级，那里
`#[cfg]` 的作用域也由此确定（规则在 `docs/zh-CN/reference.md` §9.4）：

```rust
# use batch_impl::batch_impl;
# struct Pair<A, B>(A, B);
# trait Tag { fn tag(&self) -> u32; }
#[batch_impl(A : [u8, u16])]      // stage 1 绑定 `A`
#[batch_impl(B : [u32, u64])]     // stage 2 绑定 stage 1 留下的 `B`
impl Tag for Pair<A, B> { fn tag(&self) -> u32 { 0 } }
// → impl Tag for Pair<u8,u32> / Pair<u8,u64> / Pair<u16,u32> / Pair<u16,u64>
```

**顺序为什么是必需的，而不只是约定**（规则在 `docs/zh-CN/reference.md` §9.5）。一个 *shape family*——头部形状各不相同的容器形态（`Vec<T>`、`[T; 4]`、`Box<[T]>`、`&[T]`）——在 §8.4 的模式里每族都需要一个 prototype，因为单个模板无法匹配四种不同形状的头部。用两步就能直接表达：第 1 步引入**留着元素槽的形状**，第 2 步填这个槽；而第 2 步的替换会**钻进**第 1 步产出的 token 内部（`B` 落在四个不同位置，其中一个在引用之后）：

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

### 9.1 元组生成器、元数与幂后缀

本节的 fresh 指宏生成的泛型参数，如 `P0`、`P1`。具体生成器与泛型生成器
可能覆盖同一个目标，尤其在参数数量与类型结构相同时；Rust 会报告 E0119
实现冲突。组合生成器前应检查目标是否重叠，按类型结构或元数拆分。

生成器里的列表仍遵循分发规则：`(X, [A,B]).N` 产生的组合会继续展开
其中的列表，再生成最终目标。

四种拼写，全部实测：

| 拼写 | 生成什么 | 例子 |
|---|---|---|
| `()N` | **N 个 fresh 参数**（生成器）——由载体决定怎么拼 | `Pair3<*[].2>` → `impl<P0, P1> … for Pair3<P0, P1>` |
| `*[].N` | 同一个生成器**被拼入**，于是载体可以追加它的参数 | `T.*[].2` → `<P0,P1>T<P0,P1>` |
| `(A, B,)N` | 元素的 **N 重笛卡尔积**（长度 N 的元组） | `(u8, u16,)2` → 4 个 impl |
| `().1..=M` / `(A,)L..U` | **每个元数**一个 impl，各自带自己的 fresh 参数（README 表里的 "ranges"） | `().1..=3` → `impl<P0> … for (P0,)`、`impl<P0,P1> … for (P0, P1,)`、`impl<P0,P1,P2> … for (P0, P1, P2,)`；超出该族**不会**由宏报出 —— 读者只会在元组上拿到裸 `E0599`，其 help 还建议同名字段方法（实测） |

幂是 **`.N` 后缀**（`(u8, u16).2` = 四个元组 impl）；并置形式 `()N` / `(u8, u16)2` 同样接受，而旧的 `^` 拼写会被拒绝并给出退休消息（§12）。后缀绑定到它所在的那个块，所以 `Box.*[].2` 是把生成器应用到 `Box`，而不是别的什么。

```rust
# use batch_impl::batch_impl;
#[batch_impl((u8,)3)]
trait T {}
// → impl T for (u8, u8, u8,) {}   （一元元组的三重积）
```

```rust
# use batch_impl::batch_impl;
#[batch_impl(().1..=3)]
trait Arities {}
// → impl<P0> Arities for (P0,) {} / impl<P0,P1> … for (P0, P1,) / impl<P0,P1,P2> … for (P0, P1, P2,)
```

### 9.2 笛卡尔积

`[A, B] [C, D]` 全组合；splat 幂——`(*[A, B]).2` 或并置的 `*[A, B]2`——产生笛卡尔组合列表，**必须由宿主消费**：`(*[u8, u16],).2` 是那一个元组 `(u8, u16, u8, u16,)`；而裸写 `(*[u8, u16]).2` 会把每个组合各当成一个目标，重复的目标相撞（`E0119`）：

```rust
# use batch_impl::batch_impl;
# use std::rc::Rc;
#[batch_impl([Box, Rc] [u8, u16])]
trait Matrix {}
// → impl Matrix for Box<u8> {} / Box<u16> / Rc<u8> / Rc<u16>（4 项）
```

矩阵可以进一步包进容器或组合进更复杂的 spec（`([u8, u16],)2` 等）。

## 10. 修饰符大全

完整的修饰符表（`&`/`&mut`、`*const`/`*mut`、`unsafe`、`#[...]`、`!`、`self`）在 `docs/zh-CN/reference.md` §3.8；本节只留三个**读法容易搞错**的。

`&`、`*const`、`*mut`、`unsafe`、`fn` 类型、属性全支持：

```rust
# use batch_impl::batch_impl;
#[batch_impl(&str, &mut [u8], *const u8, *mut u8)]
trait Ptrs {}

#[batch_impl(unsafe fn(u8) -> u8)]
trait FnT {}

#[batch_impl(#[cfg(all())] u8)]
trait Attr {}
// → impl Ptrs for &str {} / &mut [u8] / *const u8 / *mut u8，impl FnT for unsafe fn(u8) -> u8，
//   impl Attr for u8 {}（属性会附着到生成的 impl 上——`#[repr(C)]` 在那里**不**合法）
```

> **`unsafe` 有两种角色**——`unsafe fn(A) -> B` 是 *unsafe fn 类型*：impl 本身保持安全（`impl Tr for unsafe fn(A) -> B`）。要把 **impl** 标记为 unsafe，用 `.` 应用 `unsafe`：`unsafe.fn(A) -> B` = `unsafe impl Tr for fn(A) -> B`。如果你写 `unsafe fn(...)` 却期待一个 unsafe impl，那就是写错了形式。

**`self` 前缀**是恒等前缀——`self T` = `T`。在矩阵里作"裸类型占位"：`[Box, self] u8` 生成 `Box<u8>` 与裸 `u8` 两个 impl（表达"包装 + 目标本身"）：

```rust
# use batch_impl::batch_impl;
#[batch_impl([Box, self] u8 { fn tag(&self) -> &'static str { "x" } })]
trait WrapOrBare { fn tag(&self) -> &'static str; }
// → impl WrapOrBare for Box<u8> { ... } / impl WrapOrBare for u8 { ... }
```

**`!`（never）作 fn 返回类型**：`fn(A) -> !` 合法——`!` 块没有 apply 语义，尾随 `{...}` 归属 impl：

```rust
# use batch_impl::batch_impl;
#[batch_impl(fn(u8) -> ! { fn call(&self, _: u8) -> ! { unreachable!() } })]
trait NeverRet { fn call(&self, x: u8) -> !; }
// → impl NeverRet for fn(u8) -> ! { fn call(&self, _: u8) -> ! { unreachable!() } }
```

**数组/切片类型**：`[u8; 3]` 定长、`[u8]` 切片：

```rust
# use batch_impl::batch_impl;
#[batch_impl([u8; 3], [u8], &[u8])]
trait Slices {}
```

**任意嵌套类型原生支持**：`HashMap<String, Vec<(u8, u16)>>`、`Result<Box<dyn Fn(u8) -> u16>, String>` 等任意组合直接书写、结构化解析——DSL 已覆盖近乎全类型，不再是"原样透传"。

## 11. 入口

下表区分生成入口与辅助宏。`batch_impl`、`batch_impl_only` 和 `batch_trait`
提供不同的 trait／impl 输入方式；`batch_preprocess_test!` 消费开放扩展协议，
`batch_preview!` 接收带属性的 Rust item（`#[batch_impl(...)] trait … {}` 或对应的 `impl`），并不共用一种完整
输入文法。各入口的参数规则见 rustdoc，trait 路径与继承规则见参考手册 §9。

| 入口 | 形态 | 说明 |
|---|---|---|
| `#[batch_impl]` | 属性宏，挂在 `trait` 定义上 | 重发 trait 定义 + 生成 impl |
| `#[batch_impl]` | 属性宏，挂在 `impl` 块上（**impl 入口**，0.8.0） | 从一个手写 impl × 形状模板批量实例化 |
| `#[batch_impl_only]` | 属性宏，挂在 `trait` 定义上 | 只生成 impl，trait 来自外部（改名前缀 `# path::To::Trait:` —— 它**写在属性列表最前面、只写一次**：前缀属于整条属性、不属于某条 spec，逐 spec 重复只会得到一条误导性的 “`#std` must be followed by `(args)`/`[args]` or a code block `{body}`”） |
| `batch_trait!` | 函数式宏 | 分段 + 自定义 `@name=值;` 常量段；**不支持** `#` 指令 |
| `batch_preprocess_test!` | 开放扩展参考宏 | 消费协议输入，生成完整 impl；旧的 impl 内输入生成关联成员 |
| `batch_preview!` | 诊断通道 | 把展开结果作为 `compile_error!` 文本打印（唯一稳定的终端通道），**并在 trait 入口报出它生成了多少个 impl**（impl 入口只打印一个 token 流，不报数量）——这是检查“这条 spec 是否生成了你想要的东西”最快的办法 |

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
# trait A {} trait B<T> {}
batch_trait! {
    @uints = @u*;
    A: @uints;
    B: <T> B<T> Vec<T>;
}
```

## 12. 错误提示

batch-impl 的错误是**编译期诊断**，尽量指向相关的用户可见 token；没有可用
来源位置时，可能指向宏调用。一次宏调用可以汇总多个独立错误，Rust 也可能
继续给出后续诊断。先处理最先出现、最具体的错误，再重新编译。常见情况有：

- **操作数缺失**：`A.` / `.A` / `,A`
- **`@N`/`@g_i` 越界或悬空引用**：`@5` 超出 impl 生成的泛型数，或 `@2_0` 组不存在——fresh 泛型从 0 按文档序编号、显示为 `P0`、`P1`……；悬空引用在宏内被拦截，绝不落为 rustc E0412 裸错
- **`where` 谓词不是合法 Rust 谓词**：`where{ A B }`（漏 `:`）在谓词定型后报错并给出修法；谓词里的 **splat** 同样报出，因为该子句到输出全程 token 级
- **`=`/`:` 写错实参表**：bound 与 binding 只属 trait 路径（`Conv<Item = u32> X`）或 **bound 位置**（`T: Iterator<Item = u8>`，`dyn` / `for<'a>` 内同理）；`<>` **声明块**声明的是参数，那里的 binding 会被报出并给出可用写法
- **`<>` 声明块里的 fresh 生成器**：把生成器写在类型上——`T.*[].2` 拼入生成的参数，`T<()2>` 把它们保持为一个元组实参
- **已退役的 `^` 幂**：`(u8, u16)^2` / `T^()^2` 有自己的消息；幂是 `.N` 后缀（`(u8, u16).2`、`T.*[].2`）

其余全部——每一类的**精确原话**与锁定它的 fixture——在 `docs/zh-CN/reference.md` §10。

## 13. 实战：仓库里那三个示例

上面每一章只讲一个机制。`examples/` 是它们**组合**成完整文件的地方，而且 CI 会编译它们，所以不会漂移。这也意味着示例不能「故意失败」，除非把它隔离：用来展示诊断的示例必须声明 `required-features`（或排除在默认目标集之外），否则 `cargo test` 会构建它，那个故意的失败就会把门禁弄红 —— 有探针为此白跑了一轮，实测：

| 示例 | 是什么 | 展示什么 |
|---|---|---|
| `examples/quickstart.rs`（约 320 行） | 可运行的单文件导览——`cargo run --example quickstart` 每个示例打印一行 `…: OK`，末尾给汇总 | 每个机制一个示例（§1–§8） |
| `examples/simplify.rs`（约 170 行） | 一个小型"数据检视"库：**30 个 impl** 出自约 15 行 DSL（手写约 80 行） | 列表 + 共享 body、包装委托、元组生成、空格应用、关联类型 binding、`#name`/`#fill`/`#delegate`、指针、三个入口 |
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

一个列表 + 一个 body → **12 个 impl**：列表展开成 impl（§3），`{…}` 中
显式写出的完整方法用于每个目标。这里没有使用签名复制指令；若要从 trait
复制签名，可以改用 §7 的 `#describe`、`#is_zero`。`Self::default()` 对每种
数值都是 0，所以一个表达式覆盖全部十二种。

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

这里三个机制交汇：trait 应用 `From<bool>` **钉住**了 trait 的参数，于是抄来的签名 `fn from(value: T)` 变成 `fn from(value: bool)`（§7.1）；splat 幂 `Frac<*(*@u*).2>` 把 `@u*` 列表喂进**两个**泛型位——6 × 6 = 36 个 impl（§4）；`#from{…}` 提供整族共用的那一个 body（§7.1）。

它上面的层级展示了这个模式的另一半：`Num` 由 `#[batch_impl]` 定义并填充，而各子类先声明、再由 `batch_trait!` 配 `@` 家族**一行一个类**地填充（§6.1）——正是 type-class 需要的形状。

### 13.3 接下来看哪里

- 机制：上面的 §1–§12，然后是[参考手册](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/reference.md)（合法性、诊断、上限）；
- API 文档：[入口与指令指南](https://github.com/5-6-1/batch-impl-rs/tree/main/src/doc)；
- 内部地图：[架构说明](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/architecture.md)。
