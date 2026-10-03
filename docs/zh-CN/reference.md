# batch-impl 参考手册

**v0.10.0 — 开发中（未发布）。** 本手册描述当前工作树中的规则系统、交叉与边界，§10 列出诊断。待发布改动见 [CHANGELOG](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/CHANGELOG.md)。

仓库源码（GitHub `main`）：[English](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/reference.md) | 简体中文

仓库链接指向公开的 `main`，可能与本地工作树不同。本地英文 rustdoc 的顶部
导航保留当前构建版本；中文资料请阅读同一源码目录中的文件。

**查阅型文档**：完整的表面、合法性矩阵、边界与保证。**学习路径**在[教程](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/tutorial.md)（从一行 impl 讲到矩阵组合）——本手册假定你已经见过 DSL 的基本形状，只回答"允许什么 / 不允许什么 / 报什么错 / 上限在哪"。

两条纪律决定了本手册的写法：

- **一条事实只有一个真相源**：教程负责"怎么写 / 为什么"，本手册负责"合法性与边界"，每条 API 的完整参数语义在 rustdoc（[API 文档源码](https://github.com/5-6-1/batch-impl-rs/tree/main/src/doc)：`batch_impl_only.md`、`batch_trait.md`、`batch_preview.md`、`directive_fill.md`、`directive_delegate.md`、`directive_blanket.md`、`directive_name.md`、`directive_open.md`、`directive_consts.md`）。三处不复制同一句话。
- **每条断言可核对**：文中"实测"指用 `batch_preview!` 或真编译量过（`cargo check` 看 rustc 诊断）；诊断措辞一律由 `tests/ui/` 的 fixture 锁定，fixture 名在本手册 §10 给出。

## 1. spec 文法

### 1.1 属性参数是一串 spec

各入口共享类型矩阵语言，但外层分隔符不同：

| 入口 | 外层形式 | 分隔符 |
|---|---|---|
| trait 上的 `#[batch_impl]` / `#[batch_impl_only]` | `#[batch_impl(u8, u16)]` | spec 之间用 `,`；非空 spec 之间的 `;` 会被拒绝（`semi_in_spec`） |
| `batch_trait!` | `batch_trait!(First: u8, u16; Second: u32);` | trait 段之间用 `;`，每段内部的 spec 之间用 `,` |
| impl 上的 `#[batch_impl]` | `#[batch_impl(Slot: u8; Slot: u16)]` | impl 入口的 spec 之间用 `;`；每个矩阵源使用共享的列表语法（§9.2） |

```rust
# use batch_impl::{batch_impl, batch_trait};
#[batch_impl(u8, u16)]
trait FromTrait {}

trait FromSection {}
trait OtherSection {}
batch_trait!(FromSection: u8, u16; OtherSection: u32);

trait FromImpl {}
#[batch_impl(Slot: u8; Slot: u16)]
impl FromImpl for Slot {}
# fn both<T: FromTrait + FromSection + FromImpl>() {}
# both::<u8>();
# both::<u16>();
# fn other<T: OtherSection>() {}
# other::<u32>();
```

| 概念 | 说明 |
|---|---|
| 空的 `#[batch_impl]` 参数 | `#[batch_impl]` / `#[batch_impl()]` 不派生 impl，保留被标注的 trait 或 impl；单独一个 `;` 也按空参数接受，但这不表示 trait 入口可以用 `;` 分隔 spec |
| 一个 spec | 一个**类型矩阵**；矩阵的每个格子生成一个 impl |
| spec 形状 | `[<泛型声明>] [trait 应用] 目标类型`，外加任意顺序的附件 |
| 附件 | `{body}`（实现体）、`where{...}`（谓词）、`impl{...}`（Self 形状模板） |

附件是**块**：它们以任意顺序与 spec 链组合，链条深度上限 128 层（ui `attach_too_deep`）。

列表元素的局部 body 与列表外的共享 body 追加合并，没有覆盖优先级。
不同成员可以共存；同名方法重复出现由 Rust 报错。特殊实现应拆成独立 spec，
其余目标再共享 body，见[教程 §1.3](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/tutorial.md#13-特殊类型用独立-spec)。

### 1.2 头与目标怎么切：元素边界 vs 路径续接

**规则**：trait 应用只有在头部之后跟着**一个新元素**（目标）时才成立；`<...>`（跟在 ident 之后）与 `::` 都是**当前路径的续接**，它们不产生新元素；空格与 `.` 是**元素边界**（同一个 apply 的两种结合性）。

因此目标以 `::` 开头时必须把这条缝显式写出来：

| spec | 生成 | 说明 |
|---|---|---|
| `@trait<u8> . ::std::string::String` | `impl Tr<u8> for ::std::string::String` | ✓ `.` 是元素边界（实测：真编译 + 运行期断言通过） |
| `@trait<u8> ::std::string::String` | `impl Tr for Tr<u8>::std::string::String` | ✗ 粘连成一条路径——trait 落进类型位置（E0782） |
| `Tr<u16> . Vec<u8>` | `impl Tr<u16> for Vec<u8>` | 有 trait 头时 `.` 与空格**等价** |
| `Tr<u8> (::some_mod::SomeType)` | `impl Tr for Tr<u8, ::some_mod::SomeType>` | 组**不是**边界——组是实参追加 |
| `::std::vec::Vec<u8>` | `impl Tr for ::std::vec::Vec<u8>` | spec 只有一个元素 ⇒ 整串是目标，trait 取被标注者 |

edition 2024 里 `::name` 指**外部 crate**；要指本 crate 根写 `crate::...`。

### 1.3 预处理顺序

固定四趟，顺序决定"什么能写进什么"：

**`@` 常量展开 → `<>` 尖括号配对 → `#` 指令展开 → `where` 处理**

- `@` 的值可以含**扁平** `<...>`（配对在它之后，因此看得见）；
- `#` 的参数可以引用 `@` 展开出的列表；
- `where` 最后看到完整结构；`impl{...}` 在它眼里是谓词区的边界（`@trait` 仍会被 `expand_consts` 展开进 `impl{...}`）。

**透传守卫**：`ident![...]` 宏体与 `#[...]` 属性内是任意 Rust，四个递归入口一律不进入——写在属性里的 `#name` 永远不是指令。

### 1.4 记号一张表

只列没有自己那一节的记号——`@`、`#`、`<>`、splat 与幂由各自的小节讲。

| 记号 | 含义 |
|---|---|
| `.` / 空格 | 同一个 apply 的两种结合性：`.` 嵌套（右结合），空格累加（左结合）；也是绝对路径目标之前的元素边界（§3.2、§1.2） |
| `[A, B]` / `[A,]` | 类型列表，每个元素一个 impl；`A` 是单个目标，`[A]` 是切片类型，`[A; N]` 是定长数组（§3.3） |
| `(...)` / `(A)` | 元组 / 透明组——被应用时算**一个**实参（§3.2） |
| `&` `&mut` `*const` `*mut` `unsafe` `self` `#[...]` `!` | 前缀与修饰符，各自作用于紧随其后的块：`self` 是恒等，`unsafe.fn(A) -> B` 标记 impl 而 `unsafe fn(A) -> B` 是 fn 类型，`!` 是返回类型（§3.5） |
| `{body}` / `where{...}` / `impl{...}` | 三种附件块，顺序自由（§7、§8） |
| `;` | 分隔 `batch_trait!` 的 trait 段或 impl 入口的 spec（§1.1）；也分隔数组的元素类型与长度（`[T; N]`） |
| `,` | 分隔 trait 入口的 spec，包括 `batch_trait!` 每个段内的 spec（§1.1），以及列表、元组、实参与指令参数的元素 |
| `-name` | 排除项，仅指令参数列表（§6.2） |
| `.N` / `()N` | 幂：`T.*[].2` 把生成的参数拼进去，`T<()2>` 把它们保持为一个元组实参。`^` **不是**算子——`(u8, u16)^2` 得到的是退休算子消息（§3.4、§10.1） |

## 2. 位置 × 构造

同一构造在不同位置**合法性不同**，因为门控是**位置**的属性，不是列表形状的属性。

| 位置 | bound `T: Clone` | binding `Item = u32` | 包 `*X` | 生成器 `().N` | `@` 引用 | `X<>` 同步 |
|---|---|---|---|---|---|---|
| trait 应用 `Conv<…> X` | ✓ | ✓（提升进 impl body，因 `impl Trait<Item=u8> for X` 是 E0229） | ✓ `Conv<*(A,B)> X` → `Conv<A,B>` | ✓（fresh 声明提升到 impl） | ✓ | ✓ |
| 泛型声明 `<…>` | ✓ | ✗ 定向错误（声明的是**参数**；给出 trait 应用的写法） | ✓ `<*(A,B)>` → `<A, B>` | ✗ 定向错误（该块**就是** impl 的参数表，fresh 永不被使用；ui `decl_generator_splat`） | ✓（`<@0..>` 声明 fresh） | ✓（`A<>` 头部展开） |
| 纯类型实参 `Vec<…>` | ✗ 定向错误 | ✗ 定向错误（ui `concrete_binding` / `concrete_bound`） | ✓ `T<*(A,B)>` → `T<A,B>` | ✓ | ✓ | ✓ |
| 内联 bound `<T: …>` | ✓ | ✓ | ✓ `<T: Tr<*(u8, u16)>>` → `<T: Tr<u8, u16>>` | ✓（`Fn.().N` 的 fresh 提升到 impl） | ✓ | ✓ |
| `dyn` / `for<'a>` 尾巴 | ✓ | ✓ | ✓ `dyn Tr<*(A,B)>` → `dyn Tr<A,B>`（实测） | ✓ | ✓ | ✓ |
| where 谓词 | ✓ | — | ✗ 由终检报错（见 §7） | — | ✓（`@N` 族） | ✓ |
| 目标类型（callable 的参数表是同一张表） | ✗ | ✗ | ✓ `fn(u8, *(u16, u32))` → `fn(u8, u16, u32)`，与元组内一致 | ✓ | ✓ | ✓ |
| `impl{...}` 模板 | — | — | ✗（模板是标准 Rust 类型，DSL 算子被 syn 拒） | ✗ 同上 | ✓（`@trait` / `@` 在 `expand_consts` 展开） | ✓ |
| body | — | — | ✗（不解释，`a * b` 保持乘法） | — | ✓（`@N`；`@{N}` 需 `impl{@{}}` 开关） | — |
| 指令参数 `#fill(…)` | — | — | — | — | ✓（`@all` 家族、`[a,b]` 列表） | — |

指令域与类型域互不进入：`#` 后只有指令名、`@` 家族标记、`,` 分隔的名字、`-[a,b]` 排除项与字面 `[a,b]` 列表；类型域算子写进指令参数不会被解释。

## 3. apply 系统

类型域只有一个算子、两种拼写；其余一切都是"块"。本节系统性给出规则——教程按例子教它们（§2、§3、§10）。

### 3.1 块

块是一个原子：路径、组 `(...)`、列表 `[...]`、元组、前缀（`&`、`&mut`、`*const`、`*mut`、`unsafe`、`self`、`#[...]`）、包（`*X`，包括 `*(...)` / `*[...]`）、生成器（`().N`）、`@` 常量的展开结果，或某条指令的产物。附件（`{body}`、`where{...}`、`impl{...}`）也是块，可以以任意顺序跟在 spec 之后（§1.1）。

### 3.2 两种拼写

| 链 | 规则 | 实测结果 |
|---|---|---|
| `A B C` | 空格**左结合、累加**进头的实参列表 | `Box Vec u8` → `Box<Vec, u8>` |
| `A.B.c` | `.` **右结合，且比空格结合更紧** | `Box.Vec.u8` → `Box<Vec<u8>>` |
| `A.B C` | 先解 `.` 链，再让空格累加到头上 | `Box.Vec u8` → `Box<Vec, u8>` |
| `A B.c` | `.` 更紧，因此它先嵌套、空格后应用 | `Box Vec.u8` → `Box<Vec<u8>>` |
| 单个块 | 原样；组是**一个**实参 | `Box (u8, u16)` → `Box<(u8, u16)>` |
| 前缀 | 作用于紧随其后的那个块 | `& Box u8` → `&Box<u8>` |

两条值得记住的推论：**要嵌套就用 `.`**（`Box Vec u8` 永远不是 `Box<Vec<u8>>`），**要传多个实参就用空格**（`Box u8 u16` → `Box<u8, u16>`）。

### 3.3 列表与元组

`[A, B] T` 把后面的类型分发到每个元素——各生成一个 impl（`[Box, Rc] u8` → `Box<u8>` + `Rc<u8>`）；裸列表作目标时就是同一件事的另一种写法。`(A, B)` 是一个元组值；`(A)` 透明；`[A]` 作类型是切片、`[u8; 3]` 是数组。列表提供**候选分支**，元组提供**同时存在的字段**；二者都可以用 `*` 打开直接成员，得到同一种包（§4）。列表及包均不自动去重。

### 3.4 幂 `.N`

幂写作 `.N`，跟在被重复的那个值后面：`T.N` 把元组或生成器展开成 `N` 个位置的笛卡尔积——`(u8, u16).2` 是 `{u8, u16}` 上的全部有序对，即 4 个 impl；`Frac.*(*@u*).2` 把 `@u*` 列表喂进两个泛型位，得到 36 个（`examples/typeclass.rs` 就是这个拼写；实参形式 `Frac<*(*@u*).2>` 给出同样的 36 个）。单 spec 的 1024 impl 上限（§12）就是用来报出打错的指数的。

`*[].N` 生成含 N 个 fresh 参数的包，由实参或元组等宿主拼入成员：`T.*[].2` 声明两个 fresh 并用在目标里（`impl<P0, P1> … for T<P0, P1>`）。

**`^` 不是算子**：`(u8, u16)^2`、`Box^*()^2`、`Box<()^2>` 一律被拒，报的是 §10.1 逐字引用的退休算子消息（`caret_power_retired`）——span 落在这个 `^` 上，并给出可用的 `.N` 拼写。**bound 位置**的 `^`（`<T: Tr^u8>`）报同一条消息。更早的文档用 `^` 写幂，请写 `.N`。

### 3.5 `self` 与裸类型占位

`self` 是恒等前缀：`self T` = `T`。在矩阵里它代表裸类型本身（`[Box, self] u8` → `Box<u8>` **和** `u8`），"包装或裸"这一类族就是这么写的。

### 3.6 apply 在哪里停

当头部就是被标注的 trait（或 `@trait`）时，第一个元素是 **trait 应用**、其余是**目标**；有 trait 头时 `.` 与空格等价。ident 之后的 `<...>` 与 `::` 续接路径，而 `.` 与空格是元素边界——绝对路径目标、以及 `Tr<T>::Type` 是一个类型，都由这条规则决定（§1.2）。

### 3.7 边界情形

| 拼写 | 会发生什么 |
|---|---|
| `A.` / `.A` / `,A` | 操作数缺失，定向报错（§10.1） |
| `(A)` 与 `A` | 同一表达式，包括包；构造元组写作 `(*X,)` |
| `[A]` 与 `[A, B]` | 切片 vs 两个 impl |
| `Box u8 u16` | `Box<u8, u16>`——两个实参，不是嵌套泛型 |
| `Box Vec u8` | `Box<Vec, u8>`——空格累加；用分组（`Box (Vec u8)`）或 `.`（`Box.Vec.u8`）可写出 `Box<Vec<u8>>` |
| `& Box u8` | `&Box<u8>`——前缀吃掉后面那个块 |
| `*(A,B)` 单独作目标 | 每个元素一个 impl；`(A,B)` 则生成一个元组 impl（冲突规则见 §4.6） |
| `HashMap<String, Vec<(u8, u16)>>` 这类嵌套类型 | 直接写、直接解析——不存在"透传"写法 |

### 3.8 前缀与属性

前缀是一个块，它会吃掉紧随其后的那个块（`& Box u8` = `&Box<u8>`）。各自合法在哪里：

| 前缀 | 含义 | 合法位置 | 备注 |
|---|---|---|---|
| `&` / `&mut` | 引用类型 | 任意类型位置 | `& Box u8`、`&str`、`&mut [u8]` |
| `*const` / `*mut` | 原始指针类型 | 任意类型位置 | 由后续 token 决定，因此绝不会被读成 包（§4.1） |
| `unsafe` | 用 `.` 应用时是 **impl** 标记；否则是 fn **类型** | `unsafe.fn(A) -> B` 标记 impl；`unsafe fn(A) -> B` 是类型 | 最容易读错的一处（教程 §10） |
| `#[...]` | 附着到生成 impl 的属性 | 附着在 spec 上 | `#[cfg(all())] u8`；DSL 绝不进入属性内部 |
| `!` | never 类型 | `fn` 返回位置 | `fn(u8) -> !`；`!` 块没有 apply 语义 |
| `self` | 恒等前缀 | spec 头部位置 | `self T` = `T`——矩阵里的裸类型占位 |
| `fn` 家族（`fn` / `Fn` / `FnMut` / `FnOnce` / async 形式） | callable 类型 | 任意类型位置 | 它的参数表就是参数位置列表（§4.4） |

## 4. 包 `*`

### 4.1 规则

`*X` 把一个块打开为**包**，透明分组不改变它。
元组或候选列表贡献直接成员；包仍是包；其他类型贡献一个完整成员。
声明载体随成员保留，打开操作不会递归进入普通类型。

因此 `*(A, B)` 与 `*[A, B]` 使用同一种包表示。
`*A` 是单成员包，`*()` 为空，`*(*X)` 等于 `*X`。
没有专门的双星操作。`*const T`、`*mut T` 优先识别为原始指针。

### 4.2 分组、元组与候选

出现 `*` 不改变括号解析：
`(X)` 是分组，`(X,)` 是元组，`[X]` 是切片，
`[X,]` 是候选列表。既有 `(@0..)` 元引用范围元组写法
仍遵循其独立规则（§5）。

| 写法 | 含义 |
|---|---|
| `(*(A, B))` | 加了分组的包；根位置生成两个目标 |
| `(*(A, B),)` | 一个元组 `(A, B)` |
| `[*(A, B)]` | 切片的元素类型槽含两个成员：报错 |
| `[*(A, B),]` | 候选中的包贡献目标 `A`、`B` |
| `*((A, B),)` | 包含一个成员：完整元组 `(A, B)` |
| `*(A, [B, C])` | 两个分支，分别含 `A, B` 与 `A, C` |
| `*(A, *[B, C])` | 一个包，含 `A, B, C` |

候选不会因为处在包内部就自动变为成员，必须显式打开才会失去分支角色。

### 4.3 应用

应用保留空格左结合与点右结合。分派顺序如下：

1. 保留声明，分派暴露的候选（先右后左）。
2. 为范围生成分支；元组或包与数字相遇时执行幂。
3. 两侧都是包：逐个取右包的直接成员作为一行，将整个左包映射到该行，
   保留嵌套结果。
4. 仅左侧是包：把左包每个成员应用于整个右侧；
   左侧嵌套包继续同一个任务，不再拆开已经选中的右行。
5. 其他情况走普通应用：`self` 返回整个右侧，泛型追加一个实参槽，
   元组追加一个元素槽。

| 写法 | 放入元组后的结果 |
|---|---|
| `(*(Vec, Box) u8,)` | `(Vec<u8>, Box<u8>)` |
| `(*Vec *(u8, u16),)` | `(Vec<u8>, Vec<u16>)` |
| `(*Pair (*(self, Vec) *().2),)` | `(Pair<T0, Vec<T0>>, Pair<T1, Vec<T1>>)` |
| `(*((),) (*(self, Vec) *().2),)` | `((T0, Vec<T0>), (T1, Vec<T1>))` |
| `(*Map *().2 *().3,)` | `(Map<T0,U0>, Map<T1,U0>, Map<T0,U1>, Map<T1,U1>, Map<T0,U2>, Map<T1,U2>)` |

普通左类型不映射：`Pair *(A, B)` 保留包作为一个实参槽，
物化后为 `Pair<A, B>`。字面量 `F<...>` 直接消费实参，
不会重新执行应用。这些是求值规则，不能作为任意替换已构造中间节点的等价律。

### 4.4 宿主物化

应用完成后，候选分支，包拼入周围的宿主。嵌套包摊平，普通元组类型保持为成员。
这个步骤不执行 apply，也不生成新的 fresh 参数。

| 宿主 | 消费规则 |
|---|---|
| 裸目标 | 包的每个成员各生成一条 impl；不去重 |
| 元组元素、泛型 / trait 实参、callable 参数 | 按顺序接收任意数量成员 |
| 引用或指针目标、切片 / 数组元素、函数返回值 | 每个分支恰好一个成员 |
| 单个 `+` bound、关联类型绑定值、限定路径中已解析的类型头部 | 每个分支恰好一个成员 |
| 声明块 `<*(A, B)>` | 拼入名字，再检查声明合法性；拒绝此处的 fresh 生成器 |
| `where{...}`、`impl{...}` 模板 | 标准 Rust 类型语法域，不解释包运算 |
| body / 指令参数 | 各自的语法域，不解释包 |

例如 `fn(*(u8, u16))` 为 `fn(u8, u16)`，
`&*u8` 为 `&u8`，而 `&*(u8, u16)` 得到定向错误。
没有分支的候选不生成内容；单类型槽选中的空包则报错。

限定路径的 `::Assoc<...>` 续接部分，以及 `<T as Trait>::Assoc` 中
`as` 后的 trait 路径，仍按普通 Rust 路径保留，不在这些部分拼入包。
这与已解析的类型头部接收包是不同的位置。

### 4.5 生成器、身份与限制

普通元组的幂操作直接槽：
`([A, B],).2` 保留四种组合，
`(*(A, B),).2` 复制一个包槽，得到 `(A, B, A, B)`。
包的幂则先拼平嵌套包与透明分组，携带声明，但保留普通元组和候选，
再使用普通幂规则；每个分支生成的元组重新成为包。

`*[].N` 生成 `N` 个独立参数，`.0` 不分配参数。
复制已生成参数保留身份；执行不同生成器创建不同参数组。
因此 `(*(),).2` 是无声明的单元元组，
`*(*(),).2` 却会生成两个参数。

宿主没有成员不代表声明被删去：
`(*Map *().2 *().0,)` 留下单元元组上的两个未使用参数（E0207）。
而 `(*(().2),).0` 在物化前丢弃整个模板，不留下声明。

通用的展开与嵌套上限仍然适用（§12）。包运算还检查累计结构工作量，
包括声明和嵌套成员，因此可能在不足 1024 条最终 impl 时触及工作量上限。
名义长度 1024 不意味着每种同长度嵌套构造都能容纳。

### 4.6 分支、重叠与迁移

`([*Vec, *Box] *().2,)` 得到两个统一分支；
`(*([Vec, Box],) *().2,)` 得到四种独立选择；
`(*(Vec, Box) *().2,)` 得到一个四成员元组。
没有在应用前冻结所有内部候选的全局步骤。

固定的二维包可以让六个成员共享五个参数。
但拼平两个*长度范围*可能产生重叠泛型 impl 模式（E0119）。
用 `(*((),) (*Map *().1..=2 *().1..=3),)` 保留行，即可保留维度。
重复目标永远不会被静默删除。

从旧 splat 行为迁移：

- `*(F, G) T` 现在映射两个构造器；旧的元组追加行为写作
  `*((F, G) T)`。
- `*[F, G].2` 现在与 `*(F, G).2` 使用相同的包幂。
- 单独的包不再让分组升格为容器。需要容器时显式写
  `(*X,)` 或 `[*X,]`。
- 嵌套普通候选保持分支意义；要收集成员则再写一层明确的 `*`。
- `T.*(A, B)` 等既有右侧拼入写法仍然可用。

[教程](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/tutorial.md)
§4 提供完整程序；
[模型契约](https://github.com/5-6-1/batch-impl-rs/blob/main/tests/pack_model/contract.zh-CN.md)
记录求值阶段与反例。

## 5. `@` 宏元层

### 5.1 规则

`@` 是**仅有的宏元记号**（`#` 只剩指令名）。替换是**词法**的：值以 token 形式被拼接，**引用处不做任何域内解析**——结果进入正常管线，在那里像手写文本一样被解析。它在四趟里**最先**跑（`@` → `<>` 配对 → `#` → `where`），这才让两件事成立：

- 值里可以含**扁平** `<...>`，因为尖括号配对在它之后；
- 值可以是另一个常量（`@a=@b`）或一整个 DSL 表达式，在引用处递归拼接并展开。

### 5.2 按类别的记号

| 类别 | 记号 | 展开成 | 细节 |
| --- | --- | --- | --- |
| 名字族 | `@u*` `@i*` `@f*` `@num` `@scalar` | 类型**列表** | 语言定义的闭集（教程 §6.1） |
| 范围族 | `@u8..u16` `@u8..=u16` `@i8..=i128` `@f32..=f64` | **列表**——`..` 排除上端点，`..=` 包含上端点 | 省略下端点从族最小值开始；省略上端点（`@u16..`）包含族最大值；`usize`/`isize` 不在任何范围族里 |
| trait | `@trait` | trait 路径（`batch_trait!` 里是该段自己的路径） | 当前入口的 trait 上下文（§5.3） |
| 输入 impl 类型 | `@Self` | 当前属性调用收到的 impl 自身类型 | 仅 impl 入口；普通 Rust `Self` 保持原义（§9.2） |
| trait 成员族 | `@all_methods` `@all_constants` `@all_types` `@all_required*` `@all_default*` `@all_ref_methods` `@all_value_methods` `@all_static_methods` | `[a,b,c]` **组**，随后进指令参数解析 | 必需/默认与接收者过滤属于常量本身 |
| 泛型参数族 | `@all_type_params` `@all_const_params` `@all_lifetimes` | 从 trait 拷来的扁平 `<...>` **声明** | const 参数带完整 `const N: usize`（裸名是 E0747） |
| 包装常量 | `@Cow` | `Cow<'_>` + 该包装的约束谓词 | 仅 `#blanket` |
| 位置引用 | `@N` `@g_i` `@0..=M` `@N..` | 一个 fresh 名，或逗号分隔的一串 | §5.4 |
| 自定义常量 | `@name=值;` | 值本身，逐字 | 仅 `batch_trait!` 的前导段 |

### 5.3 按入口看合法性

| 记号 | `#[batch_impl]` | `#[batch_impl_only]` | `batch_trait!` | 备注 |
|---|---|---|---|---|
| 名字族 / 范围族 | ✓ | ✓ | ✓ | 纯词法列表 |
| `@trait` | ✓ 本地名，或输入 impl 的 trait 路径 | ✓ 外部路径（`# path::To::Trait:` 前缀），或输入 impl 的 trait 路径 | ✓ **逐段**替换 | inherent impl 没有 trait 路径（`src/doc/batch_trait.md`） |
| `@Self` | ✓ impl 上；✗ trait 上 | ✓ impl 上；✗ trait 上 | ✗ | 复制输入自身类型；名称保留，不能自定义 |
| `@all*` 成员族 | ✓ | ✓ | ✗ 定向错误 | 它们需要 trait 定义 |
| `@all_type_params` / `@all_const_params` / `@all_lifetimes` | ✓ | ✓ | ✗ 定向错误（ui `generic_family_batch_trait`） | 从 trait 自己的参数拷贝 |
| `@Cow` | ✓（仅 `#blanket`） | ✓（仅 `#blanket`） | ✗ | 是包装打包常量，不是类型别名 |
| `@N` / `@g_i` / `@0..=M` / `@N..` | ✓ | ✓ | ✓ | 比 `@trait` 更晚解析，在 codegen |
| `@name=值;` | ✗ 定向错误（ui `const_attr_unsupported`） | ✗ 同上 | ✓ | 仅 `batch_trait!` |

### 5.4 地址

- **编号与显示名**：fresh 泛型按**文档序**是 `P0`、`P1`……，`@N` 就是这个下标（`@0` → `P0`）。用户自己写的参数用它们自己的名字——`@N` 之所以存在，正是因为 fresh 名不是用户写的。
- **`@g_i` 是本原**：组 `g`、槽 `i`，跨数组分发保持稳定；`@N` 是摊平形式。实测：`().2 where{@0_1: Clone}` → `where P1: Clone`。
- **`@N..M`** 不含 M，**`@N..=M`** 包含 M，**`@N..`** 开到最后一个 fresh；数字元数范围与命名类型族范围使用同一端点规则。在 where 谓词里，一串覆盖会变成**每个 fresh 一条谓词**：实测 `().2 where{@1..: Clone}` → `where P1: Clone`。
- **排他区间在每个位置都不含末尾**：实测 `().3 where{@0..2: Clone}` → 三 fresh 的 impl 上得到 `where P0: Clone, P1: Clone`。
- **越过末尾的开区间什么都不贡献**：实测 `().2 where{@5..: Clone}` → 没有谓词、也不报错。依赖元数的 spec 不该因为短的那个情形就失败。
- **`@N` 越过末尾是定向错误**（ui `at_num_in_type`；spec 里闭区间的对应物是 `empty_range`）。
- **blanket 包装的 where 子句里，`@0` 指目标泛型**：实测 `#blanket(own){Box where{@0: Copy}}` → `impl<P0> … for Box<P0> where P0: Trait, P0: Copy`。
- **`@all_fresh` 已移除**：将既有用法改为 `@0..`。

### 5.5 定义

值是**逐字 token**，在引用处展开。**定义处**就被拒（在任何 impl 生成之前）：循环（`@a=@a`）、前向引用（`@b` 之前就写 `@a=@b`）、裸范围端点（`@a=@u8` 无 `..`）。值内的嵌套共用 §11 的深度上限。

### 5.6 边界与交叉

| 写的 | 会发生什么 |
|---|---|
| `Box<@1.5>` | "`@` in a type must be followed by a position digit (e.g. `@0` or `@0_1`)"——只有 `@N`/`@g_i` 是引用 |
| 没有 fresh 时写 `Box<@5>` | 定向错误（ui `at_num_in_type`） |
| `Box<0>` | 裸整数在 DSL 里**就是**类型（渲染 `Box<0>`）；只有 `@` 才引入引用 |
| `@1_000` | `_` 是"组/位"分隔符而非数字分隔符：`@1_000` 是组 1、位 0——字面量在**第一个** `_` 处切开，扁平下标 1000 请写 `@1000` |
| `where{...}` 里的 `@trait` | 会展开——实测 `<T> TrW<T> u8 where{@trait<T>: Sized}` → `where TrW<T>: Sized` |
| `impl{...}` 里的 `@trait` | 会展开——实测 `Box<u8> impl{@trait<u8>}` → `impl TrI for Box<u8>` |
| `@all*` 家族作指令参数 | 指令域自己的输入——实测 `u8 #fill(@all_methods){7}` 填满该 trait 的每个方法 |
| `@Cow` 作 blanket 包装 | `Cow<'_>` + 该包装的约束谓词——实测 `#blanket(@all_methods){@Cow}` → `impl<P0> … for Cow<'_, P0> where P0: Trait, P0: ToOwned + ?Sized, …`（完整形式由 `tests/features/dsl_macro_meta.rs` 锁定） |
| `batch_trait!` 里的 `@trait` | 逐段替换成该段自己的 trait 路径，因此一段可以复用另一段的常量 |

## 6. `#` 指令

指令从 trait 定义抄签名、批量填 body、生成委托调用，或写出一整个 blanket impl。本节系统性地给出这套机制：形状、作用域文法逐元素、每条内置指令，以及每种被拒形态对应哪条消息。

### 6.1 形状，以及产物能挂在哪

`#指令名(作用域){内容}`——所有内置指令都是同一个形状：一个**名字**、一个**作用域**（作用于什么）和一份**内容**（怎么处理）。`#name{body}` 是 `#fill` 的单成员特例：`#fill([foo]){body}` ≡ `#foo{body}`，只处理一个成员时用短的那种。

产物形态决定它能站在哪：

| 产物 | 指令 | 附着到类型 | 单独作 spec |
| --- | --- | --- | --- |
| **单组** | `#name`、`#fill`、`#delegate`，以及开放扩展的 `{...}` 组 | ✓（`T {body}`） | ✓ |
| **多 token**（自带泛型、目标与委托） | `#blanket` | ✗——附着没有意义 | ✓ |

指令名后面既没有 `(args)` 也没有 `[args]` 时，`{body}` 仍然必需；光写 `#m` 是 `directive_bad_follow`："`#m` must be followed by `(args)` or `[args]` + `{body}` (or directly `{body}`)"。

delegate body 表达式中的 `receiver.#call` 是单独的局部标记，不是指令调用（§6.5）。

### 6.2 作用域文法

作用域由**指令域**解析，不是类型域（§6.8）：它是一个 `,` 分隔的元素列表。

| 元素 | 含义 | 被拒的形态 |
|---|---|---|
| `name` | 按名字选一个 trait 成员 | 成员不存在 → "item `no_such` not found in trait `T`"（`single_name_not_found`） |
| `@all` 家族 | 选中的成员集合（§5.2）：`@all_methods`、`@all_constants`、`@all_types`、`@all_required*`、`@all_default*`、`@all_ref_methods`、`@all_value_methods`、`@all_static_methods` | 在 `batch_trait!` 里用它（那里没有可选择的 trait 定义） |
| `[a, b]` | 名字的字面列表；接受 `[a, b,]` 与 `[]` | 不存在的成员名报错 |
| `-name` / `-[a, b]` | 从集合里排除；接受空结果 | `-` 后面什么都没有 → "after `-` expected an identifier or `[...]` list"（`minus_bad_target`）；不存在的排除名报错 |
| `,` | 分隔元素；接受一个尾逗号 | 拒绝前导或连续逗号（`fill_bad_comma`） |
| （什么都没有） | 不选任何成员 | 接受，包括 `#fill()`、`#delegate()`、`#blanket()` |

名字在选择、排除之前检查：`typo, -typo` 不能隐藏不存在的成员。空字面列表、结果为空的 `@all` 家族、合法地排空所有成员的差集含义相同。`#fill`、`#delegate` 不生成成员；`#blanket` 仍生成包装 impl。未实现的必需 trait 成员由 Rust 检查。delegate 改名的左侧必须是 trait 方法名；右侧是目标对象的方法名，交给 Rust 检查。

### 6.3 `#name{body}`——单个成员

找出名字为 `name` 的**那一个** trait 成员——方法、关联常量或关联类型——用 `body` 填上，body 必须匹配该成员的形状（`usize #to_str{"usize"}`）。它就是短写形式的 `#fill([name]){body}`，一次性场合的惯用选择。

### 6.4 `#fill(scope){body}`——一个 body，多个签名

对每个被选中的成员，**签名从 trait 定义抄来**，`body` 成为它的实现（`#fill([add, add2]){self.0 = self.0.wrapping_add(x as u32)}` 用一个 body 填两个方法）。这是指令系统的核心承诺——声明数据、不写重复代码——也是作用域存在的理由：一个 body，宏把它复制到每个被选中的签名之下。宏**不**对 body 做类型检查；不满足签名的 body 由 rustc 对着生成的 impl 报出。

关联类型定义保留自身的泛型参数声明和 `where` 谓词，去掉 trait 声明的结果约束：`type Item: Clone;` 配 body `u8` 生成 `type Item = u8;`，Rust 通过 trait 声明检查 `Clone`。`#name`、`#blanket` 使用相同规则；blanket 的 GAT 投影只传参数名（`Item<'a, T, N>`），不传约束或 `const` 声明。

### 6.5 `#delegate(scope){...}`——生成调用

每个被选中的方法从 trait 定义复制签名。没有可识别的 `.#call` 时，内容是
目标表达式：`fn m(&self, ...) -> R { (target).m(...) }`——跳过 `self`、
转发其余参数。因此 `Box.Vec.u32 #delegate(d_len){**self}` 生成
`fn d_len(&self) -> usize { (**self).d_len() }`。

出现可识别的 `receiver.#call` 时，整个内容成为方法体。每个标记在当前位置
生成一次完整调用，使用当前方法的目标名字并自动转发参数。不需要额外的
模板标记，转发调用无需在 `.#call` 后追加 `()`。

| 形态 | 规则 |
|---|---|
| 作用域 | 仅方法——常量或关联类型是 "`VALUE` in trait `HasConst` is not a method"（`delegate_on_non_fn`、`delegate_const`） |
| 无 `.#call` 的 target | 一个表达式，被拼进生成的调用（`**self`、字段、构造调用）；没有标记的 `match` 也仍是目标表达式 |
| `receiver.#call` | 一次自动转发参数的完整调用；接收者可为字段、方法结果或带括号的表达式 |
| `match self { Self::A(inner) => inner.#call, Self::B(inner) => inner.#call }` | 各分支调用各自接收者，异构接收者无需统一类型 |
| `inner.#call.into()` / `inner.#call.field` | 从调用结果继续链式操作 / 读取结果的字段 |
| `inner.#call()` | 调用生成的调用表达式的结果；Rust 要求该返回值可调用 |
| `foo = call_foo` | 把 trait 的 `foo` 委托给 target 的 `call_foo`：**签名保留 `foo`**，只有调用用另一个名字 |
| 改名缺一侧 | "rename `X = Y` needs identifiers on both sides"（`delegate_rename_missing_left`）；同一方法改两次名是 "method `size` is renamed twice"（`delegate_double_rename`） |

两种形式都按声明顺序显式转发方法的类型和 const 参数（`method::<T, N>(args)`），
生命周期保持推断。不会自动添加 `.await`；异步 body 可以写
`receiver.#call.await`。body 中普通名字与调用保留 Rust 含义，只有带标记
的调用使用当前方法名及改名映射。借用、移动和结果类型由 Rust 检查。
转发参数始终引用方法的参数，即使 body 的局部绑定使用了同样的名字。

识别仅限该 delegate body 的表达式；宏 token、属性负载与内嵌 item 定义
不改写，也不触发方法体模式。名为 `call` 的顶层开放扩展、普通 `.call(...)`
方法及其他指令的 body 保持原义。可编译的枚举示例见教程 §7.3。

### 6.6 `#blanket(scope){wrapper list}`——每个包装一个完整 impl

`#blanket` 围绕一个 fresh 泛型 `T` 为**每个包装写出一个完整 impl**，每个被选中的方法都通过显式限定的当前 trait 路径委托，因此 supertrait 的同名方法不会造成歧义。引用接收者使用 `<_ as Trait>::read(&**self)` / `<_ as Trait>::add(&mut **self, value)` 这样的调用，`_` 从实际 deref 目标推断，不要求它等于包装的类型参数。静态方法使用 `<T as Trait>::method(...)`，这些路径都携带当前 trait 的泛型实参。异步方法追加 `.await`；方法的类型/const 泛型通过 turbofish 显式传递，生命周期继续推断。

| 包装列表的元素 | 含义 |
|---|---|
| 一个类型形态 | 为它实现的那个包装，围绕 fresh `T`（`.`/空格链表达嵌套，如 `Box.Arc`） |
| `:N` | 到达内层值的 **deref 深度**（`Box.Arc:2`）——必须是数字，上限 128（`blanket_bad_depth`、`blanket_bad_empty_depth`、`blanket_bad_huge_depth`） |
| `@Cow` | 打包常量：`Cow<'_>` 加上 `@0: ToOwned + ?Sized` 及额外的 `@0::Owned: @trait` 约束 |

被拒的：`*const`/`*mut` 包装（deref 会不安全，`blanket_ptr`），以及方法普通参数、返回类型或泛型约束中的裸 `Self`，包括 `U: Marker<Self>`、`where U: Marker<Self>` 和 `where Self: Marker<U>`。内部类型不能与包装类型等同；诊断建议手写 `#name{...}`（`blanket_self_return`、`blanket_self_in_group`、`blanket_self_constraints`）。

接收者里的 `Self` 按 deref/借用规则处理；`Self::Assoc` 投影在参数、返回值和约束中都允许。`where Self: Sized` 及 `Self: 'a` / `Self: Sized + 'a` 这类 outlives 条件仍可使用，实际目标是否满足条件交给 Rust 检查。属性负载不会被解释为约束。按值接收者少一层 deref（`<_ as Trait>::consume(*self)`）；显式 `self: &Self` / `self: &mut Self` 使用普通引用规则。

### 6.7 开放扩展 `{! m!{...}}`

名字既不是内置指令、也不是 trait 成员的 `#name(args){body}`，会展开成**你自己的**同名函数式宏调用，并把参数、body 与 trait 定义交给它。`{! ...}` 块自 0.6.7 起**仅限顶层**——它把 spec 体前置并在顶层发出宏调用——而且必须是最后一个块（`top_level_block_not_last`、`top_level_manual_not_last`、`top_level_without_attach`）；旧的 impl 内形式 `T {m!{...}}`（无 `!`）自 0.7.2 起废弃但仍接受。

值得知道的一点：指令名**没有拼写守卫**，所以拼错的内置指令会静默变成宏调用，并以 rustc 自己的 "macro not found" 出现——先拿 §6.3–§6.6 核对拼写。

开放扩展在类型物化之前接收 `{spec}`，其中可能仍有包与候选。自定义宏可捕获 `$($spec:tt)*` 并通过 `batch_impl_only` 重新进入 DSL；不能把任意 spec 都当作 `$target:ty`。参考接收者 `batch_preprocess_test!` 只支持普通 Rust 目标与非泛型 trait。

### 6.8 边界与交叉

- **独立的语法域**：指令参数由自己那一套解析；类型域解析器绝不递归进去，指令那一趟也绝不解释 DSL 算子（§13.1）。因此在作用域里，`,` 与 `-` 就是**这里的**含义，空格不是应用，名字就是名字——`#fill(a * b)` 不是乘法。
- **`batch_trait!` 一条都不支持**：它看不到 trait 定义，而 `#fill` / `#delegate` / `#blanket` 恰恰需要它。`@all` 家族与开放扩展都是属性宏的特性。
- **`# path::To::Trait:` 不是指令**：它是 **spec 前缀**（仅 `batch_impl_only`），声明外部 trait 的真实路径——至少含一个 `::`，随后 `@trait` 与所有路径引用都用它（尾部 ident 与被标注 trait 名不同 → `path_prefix_mismatch`）。
- **与其它系统**：指令产物是一个块，因此和其它块一样服从 apply 规则（§3.1），也能与列表、附件组合；`@` 常量在指令那一趟**之前**展开，所以作用域里可以带 `@all` 家族或自定义常量的列表；`where` 在它**之后**解析，所以 `#fill` 的 body 里可以像任何 body 一样裸写 `where 谓词 { 代码 }`。

## 7. `where`

`where` 子句约束生成的 impl。除普通 Rust 谓词之外，DSL 还做三件事：**按位置替换 trait 实参**、**填充三类标记**、以及在渲染前**校验定型后的谓词**。本节给出系统。

### 7.1 形式

| 形式 | 拼写 | 规则 |
|---|---|---|
| 后缀附件 | `Trait<A> Target where{P1, P2}` | 与 `{body}`、`impl{...}` 同类的一个块，因此它们之间顺序自由 |
| 裸形式 | `Trait<A> Target where P1` 或 `Trait<A> Target where P1 { body }` | body 可省略；输入结束时，非空谓词收成 `where{...}` 附件 |
| 继承 | 被标注 trait 定义上的 `where` | 并入每个 impl（§7.2） |

谓词列表按 **depth-0 逗号**切分，因此自带逗号的 bound 不会被切开：`where{@0: Semi<Additive, Multiplicative>, @1: Clone}` 是两条谓词——尖括号组在这一趟之前就已配对。

### 7.2 继承：位置替换

trait 自己的参数与 spec 的 trait 实参**按位置**配对，而不是按名字。这决定三件事：

- 谓词里的 trait 参数按**位置**跟随，因此**参数改名没问题**：`trait Store<T, K> where T: Clone` 配 `<X, Y> Store<X, Y> usize` 生成 `impl<X: Clone, Y> Store<X, Y> for usize`（由 `features::dsl_where::subst_renamed_generics` 锁定；`features::dsl_where_rename` 覆盖改名的生命周期、`const` 参数与多参数 trait）；
- **单类型参数谓词**（`T: Clone`）并进该参数的**内联 bound**；其余谓词带替换逐字通过（`HashMap<T, K>: Send` → `HashMap<X, Y>: Send`）；
- trait 的**内联**参数 bound（`trait B<T: IntoIter>`）以同样方式、经同一套位置映射继承。

### 7.3 三类填充源

| 标记 | 来自 | 规则 |
|---|---|---|
| `Trait<>` | 本 spec 的 trait 实参 | `X<>` 同步（§13.2）；谓词可以在任何允许类型的位置携带该标记，包括 bound 内部 |
| `@N` / `@g_i` / `@N..=M` / `@N..` | impl 的 fresh 泛型 | `@N..` 展开成**每个被覆盖 fresh 一条谓词**——两 fresh 的 impl 上 `where{@0..: Clone}` 渲染 `P0: Clone, P1: Clone`；越过末尾则**不贡献**任何谓词（实测，不报错） |
| `impl{...}` 槽位 | 形状映射 | 槽位与其它位置一样被替换进谓词（§8.3） |

### 7.4 终检

所有填充跑完后，谓词被当作 **Rust 谓词**解析；失败由 DSL 报出（§10.8），而不是混在 impl 的 token 里交给 rustc：

| 形态 | 报出什么 |
|---|---|
| 漏 `:`——`where{ A B }` | "a where predicate must be a Rust predicate — write `T: Bound` (a missing `:`, `T Clone`, is the usual cause); a `*(…)` splat is not expanded inside a predicate, so write the types out"（`where_not_a_predicate`） |
| 谓词里的 splat——`(*(A,B)): Trait`、`X: Trait<*(A,B)>` | 同一条消息：没有任何阶段展开谓词里的 splat，所以由终检报出 |
| 裸 splat 主体——`where{*[A, B]: Trait}` | "a splat cannot be a where-predicate subject (`*[A, B]: Trait`) — a `*(…)` list is a parameter position, and a predicate is a constraint, not a list; write the predicates out (`A: Trait, B: Trait`)"（`where_splat_bad`） |
| 空排他区间——`where{@2..2: Clone}` | "empty exclusive range `@2..2` (start not below end)"（`where_empty_exclusive_range`） |

### 7.5 边界情形

| 形态 | 会发生什么 |
|---|---|
| `where{T: Clone,}` | 尾随逗号被接受（实测：渲染成 `where T : Clone`） |
| `where{}` | 合法——impl 就是没有 `where` 子句（实测） |
| 输入末尾的 `where T: Clone`，后面没有 body 块 | 合法——等价于 `where{T: Clone}` |
| 输入在裸 `where` 后立即结束 | `where_missing_body`——裸关键字后没有谓词 |
| 两 fresh 的 impl 上 `where{@5..: Clone}` | 不产生谓词、不报错（实测） |
| 两 fresh 的 impl 上 `where{@5: Clone}` 或 `where{@0..=5: Clone}` | 越界错误："`@5` is out of range — this impl has 2 fresh generics (numbered from 0 in document order; user-written params are addressed by name)"（`at_num_in_type`）；闭**区间**越过末尾报同一类（`at_range_in_type`） |
| 一条谓词里有生命周期 / `for<'a>` / 投影 / 多个 bound | 普通 Rust——终检按普通 Rust 解析 |

### 7.6 交叉

- **× `<>`**：同步会把谓词里的 `Trait<>` 填上，和它在其它类型位置做的一样（§13.2）。
- **× 模板**：形状映射会被替换进谓词，变长段的段名出现在谓词里也不会破坏 depth-0 切分（`features::shape_template_boundary`）。
- **× 指令**：**blanket** 包装的 `where` 子句是唯一一处 `@0` 指**目标泛型**（blanket 唯一的 fresh）、而不是 impl 第一个 fresh 的地方——`#blanket(@all_methods){Cow<'_> where{@0: ToOwned + ?Sized, @0::Owned: @trait}}`（`features::dsl_macro_meta`）。
- **× impl 入口**：那里的继承源是手写 impl **自己的** `where` 子句，其中的占位符照常被重写（`features::impl_entry_basic`）。
- **× 入口上的 `@` 选择器**：`where @0..: SomeTrait` 约束 spec 的生成器声明的每一个 fresh（`features::impl_entry_basic`）。

系统级交叉（趟顺序、`@` × `<>`、`#` × 类型域等）集中在 §11。

## 8. `impl{...}` 形状模板

`impl{...}` 附件是**这个 impl 自身形状的原型**：它给目标类型的各个部位起名，让 spec 其余部分（以及 body）能提到它们，一个原型覆盖一整个形状族。本节系统性地给出这套机制。

### 8.1 解析点

模板里是**标准 Rust 类型**（`impl{Container<U>}`）：里面的 DSL 算子被拒——"the `impl{...}` template is not a standard Rust type (DSL operators are not allowed inside)"（`impl_template_dsl_ops`、`impl_template_range_constant`）。它**只解析一次**，就在 `X<>` 同步之后；这个顺序在两个方向上都成立：同步前模板里的 `X<>` 标记（`impl{GenW<>}`）不是合法 Rust，同步后形状匹配面对的是类型而不是 token。`@trait` 与 `@` 常量在常量阶段（最早那一趟，§13.1）进入模板，而 `where` 那一趟把它当作谓词区边界——它后面的 `where{...}` 不属于模板。

### 8.2 逐位匹配

模板与**叶子目标类型**按类型形态结构化递归比对：

| 该位置：模板 vs 目标 | 结果 |
|---|---|
| ident 与目标**相同** | 字面——原样保留；同名不能同时要求不同的替换 |
| ident **不同** | **槽位**，绑定到目标在该位置的那棵子树 |
| 一个 spec 里有多个模板 | 合并成**一份映射**——同形重复合法且冗余，冲突是 `impl_inconsistent_binding` |
| 模板无法解构的形状 | `impl_shape_mismatch` 并指出形状：元数/种类不同、fn 修饰不兼容（`impl_shape_fn_qualifiers`）、生命周期实参不同（`impl_shape_lifetime_arg`）、重复的变长段（`impl_shape_varseg_duplicate`）、段不在元组里（`impl_shape_varseg_outside_tuple`）、各段长度不齐（`impl_shape_varseg_uneven`） |

哪些形态会绑定：

| 模板形态 | 行为 |
|---|---|
| `T`（裸 ident） | 绑定整棵叶子子树（`impl{T}` 对着 `i32` → `T := i32`） |
| `Rc<T>` / `std::rc::Rc<T>`（路径，可多段） | 基名与各段 ident：相同 → 字面，不同 → 槽位；泛型实参递归（`impl{Rc<T>}` 对着 `Rc<i32>` 只绑 `T`） |
| `&A` / `&mut A` / `*const A` / `*mut A` | 引用/指针的形状是结构性的；元素绑定 |
| `[A]`（切片）、`(A, B, C)`（元组） | 元素逐位绑定 |
| `[A; 3]`（字面长度） | 长度逐字比较；元素绑定 |
| `[A; N]`（const 参数长度） | 长度**绑定**到叶子的长度（`N := 3`；body 里可以用 `N`） |
| `Wrap<N>`，且已声明 `const N: usize` | 将 `N` 绑定到 `2` 等 const 实参；由参数声明区分 const 名与类型名 |
| `fn(A) -> B` | 参数与返回类型递归匹配；安全性、ABI、可变参数形态与绑定生命周期必须匹配 |
| `[A; ()]` | **保留形态**（数组长度 `()` 在可编译代码里不可能存在）——变长段的标记，不要手写 |
| `Cow<'_, A>` | `'_'` 是**通配**，匹配任意生命周期；`'a` 与 `'b` 逐字比较；类型实参绑定 |
| `_` | **通配**，匹配任何东西并保持 `_` |

不可绑定——逐字比较，不匹配时给定向诊断而不是静默错绑：

| 模板形态 | 为什么 |
|---|---|
| **trait object** 模板里的槽位（`dyn A + Send`） | 逐字比较：只有完全相同的模板才匹配自己 |
| **跨类实参**（`Cow<'_, A>` 对一个单实参 `Box<u8>` 叶子、`Foo<A>` 对 `Foo<3>` 且 `A` 不是已声明的 const 参数） | 生命周期或 const 实参不能绑到类型实参，元数不同也对不齐——按形状族各写一个原型 |

所以模式读作"相同 ⇒ 字面，不同 ⇒ 槽位"：`impl{Container<U>}` 对着 `Vec<i16>` 目标会让 `Container` = `Vec`、`U` = `i16`；而模板里重复目标自己的 ident，就把那个位置钉住。

### 8.3 替换到达哪些面

映射对原型的 **`where` 谓词**与 **body** 应用一次；impl 入口还会改写输入块的**自身类型**与 trait 实参。trait 入口的矩阵叶子已经是最终目标，不会再次映射。槽位是**子树**：它的值拼接到名字出现的位置（`Vec<i16> impl{SlotBox<T>} where{Vec<T>: Clone}` 渲染成 `where Vec<i16>: Clone`）。拼入的值不会递归替换，因此 `u8 → u16, u16 → u32` 是合法的同时映射。同一个源名字若要求两个不同结果，则报冲突；其中也包括某个位置要求该名字保持字面的情形。

### 8.4 变长段与重复块

| 拼写 | 位置 | 作用 |
|---|---|---|
| `A@..` | 模板里 | **变长段**：匹配该形状族剩余的位置，并驱动 body |
| `@(…@0,)..` | body 里 | **重复块**：每个被覆盖元素一轮，`@ident` 拼接该轮的子树（`$(…)*` 语义） |
| `impl{@0..}` | 模板 | **fresh 绑定开关**：为纯游标块每轮绑一个 fresh，并启用 `@{N}` 引用 |
| `impl{@{}}` | 模板 | **body 槽开关**：在重复块否则会把 `@` 读成块首的地方启用 `@{N}` |

典型形状：一条带变长段的模板覆盖一个元组族的全部元数（`().1..=4 where @0..: Magma impl{(A@..)} #combine{…}`），body 的重复块逐轮写出该轮的元素。

### 8.5 边界

| 形态 | 会发生什么 |
|---|---|
| body 里的裸 `@` | "`@` inside an impl body must start a repeat block `@(...)..`"（`impl_shape_repeat_bare_at`） |
| 纯游标块、有多个模板却没选驱动 | "a cursor-only repeat block needs a driving segment"（`impl_shape_repeat_cursor_multi`） |
| 纯游标块、连开关都没有 | 以解析错误到达 rustc（`impl_shape_repeat_no_driver`，记录在 §10.6） |
| 各段长度不等 | "repeat block segments have different lengths (2 vs 3)"（`impl_shape_repeat_unequal`） |
| 引用了模板没声明的段 | "repeat block references unknown variadic segment `@X`"（`impl_shape_repeat_unknown`） |
| 一个块里两个不同驱动 | "repeat block driver `@A` conflicts with the inner segment reference `@B`"（`impl_shape_repeat_driver_conflict`） |
| 开关区间覆盖不到 fresh（`impl{@2..1}` / `impl{@2..=1}`） | "invalid fresh-binding switch — the range covers no fresh"（`impl_shape_repeat_invalid_switch`、`impl_shape_repeat_invalid_switch_closed`） |
| 模板不是标准 Rust 类型 | §8.1 的模板解析诊断；不再用该无效模板生成 impl |

### 8.6 交叉

- **× `@` 常量**：在常量阶段就被展开进模板，也就是在它被解析**之前**（§13.1）——所以 `impl{@trait<>}`、`@all_type_params` 声明或自定义常量的列表，在模板必须成为合法 Rust 时都已就位。
- **× `X<>` 同步**：同步填上模板里的空尖括号，这也正是解析点排在它后面的原因（§8.1）。真的带 trait 应用的模板（`impl{Tr<>}` / `impl{@trait<>}`）还会打开 **body** 同步；没有这种开关时，body 里的 `X<>` 留给 rustc（E0107）。
- **× `where`**：那一趟把 `impl{...}` 当作谓词区边界，随后形状映射也会替换进谓词（§8.3）——槽位在类型可用的任何地方都合法，谓词内部也是。
- **× impl 入口**：入口（`#[batch_impl]` 挂在 `impl` 块上）就是"形状模板 × 矩阵源"——一个手写原型 impl 加实例化它的 spec（§9）。
- **× apply 系统**：模板是一个块，因此像其它附件一样参与 spec 链，并能与 `{body}` / `where{...}` 以任意顺序附着（§1.1）。

## 9. 入口

各入口共享类型矩阵语言，外层分隔符见 §1.1。本节分别说明 trait 路径、impl 实例化及多阶段展开。

初次选择入口时，先读[教程 §11 的对照表](https://github.com/5-6-1/batch-impl-rs/blob/main/docs/zh-CN/tutorial.md#11-入口)。

### 9.1 各入口的规则

- **`#[batch_impl]` 标注已有 impl** 时直接复用完整实现，本地或外部 trait 都无需签名镜像。非空 spec 生成的 impl 替换原块；需要保留原类型的实现时，将它列入目标。字段、方法、构造和约束仍须对各目标成立。
- **`#[batch_impl_only]`** 接收既有 trait 的签名镜像，并从输出中丢弃这份声明。提供的签名、泛型和约束需要与真实 trait 手工保持同步；宏不会读取依赖中的定义。Rust 检查生成的 impl，而非镜像是否完整一致：上游后来新增默认方法，不一定会触发错误。
- **`# path::To::Trait:`** 是 spec 前缀而不是指令：它为 `batch_impl_only` 声明外部 trait 的真实路径，要求至少一个 `::`，随后 `@trait` 与所有路径引用都用它。尾部 ident 与被标注 trait 名不同则是 `path_prefix_mismatch`。
- **`batch_trait!`** 支持分段、自定义 `@name=值;` 定义，**不支持** `#` 指令——它看不到 trait 定义。
- **属性入口上的空 spec 列表**（`#[batch_impl]`、`#[batch_impl()]`、`#[batch_impl(;)]`）原样重发该 item：属性是派生 impls，没有可派生的就意味着原样。（impl 入口同理，见 §9.4。）

### 9.2 impl 入口的两种 spec 形态

| 形态 | 拼写 | 含义 |
|---|---|---|
| 形状形态 | `A<B> : [Box, Rc] [usize, isize]` | `模板 : 矩阵`——矩阵每个叶子与 `:` 前的模板匹配；所得槽位改写整个块，块的 for-type 不必与模板同形 |
| 直接形态 | `<T> Box<T>` | 泛型声明 + for-type，供单 spec 场景 |

`;` 分隔多个 spec（`A : u8; A : u16`）；空 spec 列表是恒等（§9.1）。

要用输入块的自身类型作为模板，直接写 `@Self`：

```rust
# use batch_impl::batch_impl;
# use std::rc::Rc;
trait Maximum { fn maximum() -> Self; }
#[batch_impl(@Self: [Box, Rc] @u8..=u16)]
impl Maximum for Box<u8> {
    fn maximum() -> Self { Box::new(u8::MAX) }
}
# assert_eq!(*<Box<u8> as Maximum>::maximum(), u8::MAX);
# assert_eq!(*<Box<u16> as Maximum>::maximum(), u16::MAX);
# assert_eq!(*<Rc<u8> as Maximum>::maximum(), u8::MAX);
# assert_eq!(*<Rc<u16> as Maximum>::maximum(), u16::MAX);
```

`@Self` 在常量展开阶段复制本次属性调用收到的自身类型，适用于该阶段原本可达的位置：模板、矩阵、泛型实参和 where 谓词。因此输入 impl 为 `u8` 时，直接形态 `Vec<@Self>` 就是 `Vec<u8>`。复制的 token 仍参与后续形状映射，不会被冻结。普通 Rust `Self` 保持 Rust 原义，body、宏调用与后续属性保留已有透传边界。堆叠属性各自读取本层输入（§9.4）。trait 属性与 `batch_trait!` 没有输入 impl 类型，会拒绝该常量；也不能自定义同名的 `@Self` 常量。

显式模板仍适合描述块内的多个位置，例如自身类型的实参与 trait 实参；它不必与输入自身类型同形。

### 9.3 impl 入口允许什么、保留什么

- `@trait`（块自己的 trait 路径）与 `@Self`（输入自身类型）沿已有常量阶段展开。inherent impl 支持 `@Self`，但没有 `@trait`。**自定义常量定义与 `#` 指令在这个入口被拒**（`implentry_hash_banned`、`const_attr_unsupported`）。
- spec 里的生成器会把 fresh 参数提升到 impl 上，`@N..` where 选择器据此解析（没有生成器时 `@N` 无从指代，报越界）。
- 块自己的泛型、`where` 子句与 `unsafe` 都保留；它的 `where` 区域在 depth-0 `;` 或输入结束处终止。

已声明的 const 参数可以直接特化。被绑定的参数在替换其引用时一并移除
声明；未变化的参数仍保持泛型：

```rust
# use batch_impl::batch_impl;
struct Bytes<const N: usize>([u8; N]);
trait Width { fn width() -> usize; }
#[batch_impl(@Self: [Bytes<2>, Bytes<3>])]
impl<const N: usize> Width for Bytes<N> {
    fn width() -> usize { N }
}
# assert_eq!(<Bytes<2> as Width>::width(), 2);
# assert_eq!(<Bytes<3> as Width>::width(), 3);
```

函数指针原型同样使用普通 Rust 类型，例如
`fn(u8) -> u16: [fn(u8) -> u16, fn(u16) -> u32]`。参数标签不是槽位；
匹配保留调用约定与生命周期结构（§8.2）。

### 9.4 impl 入口：堆叠属性是阶段

块上方第二个（第三个……）`#[batch_impl]` **不是**另一个 spec 列表。rustc 先展开最外层属性并把其余交给它；入口会把这些属性重新发射到它派生出的每个 impl 上，下一阶段再在那些 impl 上展开。于是各阶段按**源码顺序**（自上而下）作用在**不断累积的块**上，某个阶段留下的槽位由下一阶段绑定，各阶段组合成乘积。**空阶段是恒等**——可以关掉的一个阶段。

写在两个阶段之间的普通属性属于它所在的**展开层级**：它被发射到该阶段派生的 impl 上，更后面的阶段从这些 impl 继承它。这也决定了那里 `#[cfg]` 的作用域——某一层的 `#[cfg]` 会门控该层派生出的 impl **以及其下每一个阶段**。

### 9.5 impl 入口：为什么阶段顺序要紧

一个**形状族**（头部并不相同的容器形态：`Vec<T>`、`[T; 4]`、`Box<[T]>`、`&[T]`）每个族需要一个原型，因为单个模板无法匹配四种不同形状的头部。两个阶段能直接表达：阶段 1 引入形状并把元素槽位留空，阶段 2 填上该槽位，且阶段 2 的替换会深入阶段 1 的产物内部。交换两者就会失败——元素在块还没提到它时就被绑定，而形状阶段又引入一个没人绑定的槽位（实测：四个 `E0425`，每个形状叶子一个）——由 `features::impl_entry_chain` 锁定。

## 10. 诊断目录

本节记录**编译期**诊断。宏自身的错误尽量指向最接近根源的用户可见 token（宏生成物 fallback 宏调用行）；独立 spec 的错误可以一起报告，不承诺只产生一条诊断。宏展开失败或生成代码不满足 Rust 规则时，rustc 还可能在调用处等位置报告后续错误。措辞由 `tests/ui/` 的 fixture 锁定，`cargo test --test ui` 逐条核对；**每个 fixture 都在下面出现**（漏一个会让守卫测试失败）。

**来源**列说明这条消息是谁写的：**DSL** = 宏自己的用户语言诊断；**rustc** = Rust 拒绝生成或保留的代码（包含有意交给 Rust 检查的边界及已知泄漏）；**macro** = `batch_trait!` 前端自己的解析错误；**channel** = `batch_preview!` 的输出。

### 10.1 类型与 spec 语法

| fixture | 触发 | 锁定的措辞 | 来源 |
| --- | --- | --- | --- |
| `array_and_punct` | `[u8; 3; 4]` / `[u8;]` | batch-impl: array length `[T; N]` missing or malformed (write `[u8; 3]`) | DSL |
| `leading_comma` | `,A` | batch-impl: spec list cannot start with `,` | DSL |
| `dangling_operator` | `A.` | batch-impl: missing operand after `.` (e.g. `T.U`) | DSL |
| `leading_operator` | `.A`，以及前导 `-`（`-usize`、`Vec<u8>, -u16`） | batch-impl: `-` is no longer a type operator (write `A B` or `A.B`; the `-` exclusion only works in directive argument lists like `#fill(@all, -foo)`) | DSL |
| `num_as_left_operand` | `0.T` | batch-impl: number `0` cannot be a left operand; use it on the right (e.g. T.0) | DSL |
| `literal_and_range` | `1.5` / `1..x` | batch-impl: a bare literal in a type position must be an integer (usize); float/string/char literals are not types | DSL |
| `decl_generator_splat` | `<*().3> Vec<u8>` | batch-impl: a fresh generator cannot be declared here — the `<>` block declares the impl's own parameters, so its freshs would be declared and never used; write the generator on the type instead (e.g. `T.*[].2`) | DSL |
| `semi_in_spec` | 类型后多写 `;` | batch-impl: unexpected `;` after the type | DSL |
| `plus_at_type_start` | `+A` | batch-impl: `+` is not valid at the start of a type (it belongs in a bound, e.g. `T: Clone + Send`) | DSL |
| `caret_power_retired` | `(u8, u16)^2`、`<T: Tr^u8>` | batch-impl: `^` is no longer a type operator (the power is the `.N` suffix — write `(u8, u16).2` for a tuple and `T.*[].2` for a generator) | DSL |
| `star_misuse` | 裸 `*` | batch-impl: `*` needs a type block (write `*T` or `*[A, B]`); raw pointers use `*const T` or `*mut T` | DSL |
| `star_non_type` | `*1`（字面量操作数） | batch-impl: `*` needs a type operand — a literal, range or lifetime is not a type (write `*T` or `*[A, B]`) | DSL |
| `pack_zero_targets` | `*Vec *[]`（没有任何目标的 spec） | batch-impl: this spec expands to zero impls — a star over an empty list (`*[]`, `*[].0`) has no members; write the targets out or drop the spec | DSL |
| `pack_single_slot` | 单类型槽收到零个或多个类型；`<*(Vec<u8>,)>` 将构造类型用作参数声明；或 10 个独立候选槽的嵌套结构累计复制超限 | batch-impl: this type position requires exactly one type; the pack expands to 2 types（空包为 0 types）；声明错误：batch-impl: a generic declaration requires a parameter name (`T`, `'a`, or `const N`), not a constructed type；工作量错误：batch-impl: materialization work limit exceeded; simplify the nested candidates | DSL |
| `pack_flat_overlap` | `(*Map *().1..=2 *().1..=3,)` 的扁平类型族重叠 | conflicting implementations of trait `FlatFamily` for type `(Map<_, _>, Map<_, _>)` | rustc E0119 |
| `pack_unused_axis` | `(*Map *().2 *().0,)` 保留未受约束的第一轴参数 | the type parameter `P0` is not constrained by the impl trait, self type, or predicates | rustc E0207 |
| `pack_bare_fresh` | `*().2` 逐成员发出目标，但保留完整声明 | conflicting implementations of trait `BareFresh`；并报告未受约束的参数 | rustc E0119 / E0207 |
| `pack_duplicate` | `*(u8,u8)` 不去重 | conflicting implementations of trait `DuplicateTargets` for type `u8` | rustc E0119 |
| `pack_shared_identity` | 同位置的 Pair 参数不相同，或整体包装选择被混用 | the trait bound `(Pair<u8, Vec<u16>>,): SamePosition` is not satisfied | rustc E0277 |
| `extern_fn_stray_hash` | `extern "C" fn` 后接 `#(x)` | batch-impl: unexpected `#` in a type position | DSL |
| `lifetime_as_operand` | `'a T` | batch-impl: a lifetime cannot be an apply operand (`'a` belongs in bounds like `T: 'a`, declarations like `<'a>` or references like `&'a T`) | DSL |
| `qualified_tail_dsl_token` | `Foo<T>::Assoc<@0>` | batch-impl: a `::`-tail segment is a plain Rust path — DSL tokens (`@…` / `#…`) are not allowed there | DSL |
| `global_path_no_ident` | 结尾的 `::` | batch-impl: `::` must be followed by a path segment identifier (e.g. `::std::vec::Vec`) | DSL |
| `path_prefix_mismatch` | `# path::Other: Trait` | batch-impl: path prefix `#...Other` has a trailing ident that differs from the trait name `MyTrait`; the two must be identical | DSL |
| `group_angle_bare` | `(...)` 里的 `<...>` | batch-impl: a generic declaration `<...>` inside `(...)` needs the trailing-comma tuple form `(<T: Bound>,).N` | DSL |
| `bare_impl_trait_target` | 目标位置的 `impl Trait` | batch-impl: a bare `impl` in the spec is a shape template — an `impl <trait-object>` target type is not supported; write the trait object directly (e.g. `dyn Fn() -> u8`) or use an `impl{...}` template | DSL |
| `error_aggregation` | 一个属性里多个坏 spec | batch-impl: number `0` cannot be a left operand; use it on the right (e.g. T.0) | DSL |
| `trait_path_no_ident` | `batch_trait! { 1: ... }` | batch_trait! expects an ident as the trait name | macro |
| `only_semicolon` | `batch_trait! { ; }` | batch_trait! expects a trait name | macro |
| `missing_colon` | `batch_trait! { Tr ... }` | batch_trait! expects ':' to separate the trait name and impl-specs | macro |
| `unclosed_angle` | `Vec<u8>` 没有 `>` | batch-impl: unclosed `<` (missing matching `>`) | DSL |
| `range_left_operand` | `0..3.u8` | batch-impl: range `0..3` cannot be a left operand; it goes on the right (e.g. T.0..3) | DSL |

### 10.2 深度上限

| fixture | 触发 | 锁定的措辞 | 来源 |
| --- | --- | --- | --- |
| `deep_nesting` | 129 层嵌套组 | batch-impl: nesting depth exceeds 128 levels (perhaps an accidental extra bracket) | DSL |
| `nested_bracket_too_deep` | 130 层 `[` 组 | batch-impl: nesting depth exceeds 128 levels (perhaps an accidental extra bracket) | DSL |
| `chain_too_deep` | 129 层运算符链 | batch-impl: operator chain exceeds 129 levels (limit 128); split the chain into separate impl-specs | DSL |
| `segments_too_deep` | 129 层空格应用链 | batch-impl: space-application chain exceeds 129 levels (limit 128); split the chain into separate impl-specs | DSL |
| `attach_too_deep` | 129 个附件 | batch-impl: space-application chain exceeds 129 levels (limit 128); split the chain into separate impl-specs | DSL |
| `impl_attach_too_deep` | 同样的链走 impl 入口 | batch-impl: space-application chain exceeds 129 levels (limit 128); split the chain into separate impl-specs | DSL |
| `const_value_deep_nesting` | 常量值嵌套 129 层 | batch-impl: nesting depth exceeds 128 levels in a constant value (perhaps an accidental extra bracket) | DSL |
| `delegate_call_depth` | delegate body 嵌套超过 128 层组 | batch-impl: nesting depth exceeds 128 levels in a delegate template (perhaps an accidental extra bracket) | DSL |

### 10.3 `@` 常量、引用与范围

| fixture | 触发 | 锁定的措辞 | 来源 |
| --- | --- | --- | --- |
| `const_unknown` | `@unknown` | batch-impl: unknown @ constant `@unknown`; built-ins: `@u*` `@i*` `@f*` `@num` `@scalar` and ranges `@u8..u128` `@..u128` `@u16..` | DSL |
| `const_cycle` | `@a=@a` | batch-impl: constant `@a` references unknown `@a` (undefined or defined later; inside a constant definition, only built-in constants or previously defined constants can be referenced) | DSL |
| `const_forward` | `@b` 之前引用 `@b` | batch-impl: constant `@a` references unknown `@b` (undefined or defined later; inside a constant definition, only built-in constants or previously defined constants can be referenced) | DSL |
| `const_bare_endpoint` | `@a=@u8`（无 `..`） | batch-impl: constant `@a` references unknown `@u8` (undefined or defined later; inside a constant definition, only built-in constants or previously defined constants can be referenced) | DSL |
| `const_range_bad` | `@u32..u8` | batch-impl: range start is greater than end: `u32..u8` | DSL |
| `const_range_empty` | `@u8..u8` / `@..u8` | batch-impl: empty exclusive range `u8..u8` (start not below end) | DSL |
| `const_reserved_all` | `@all = ...` | batch-impl: constant name `@all` is a reserved `@all` selector; please rename | DSL |
| `const_attr_unsupported` | `#[batch_impl]` 上写自定义 `@name=值;` | batch-impl: custom constants are not supported by `#[batch_impl]` / `#[batch_impl_only]` — write the type matrix directly with `.` / space / `*` instead | DSL |
| `const_self_without_impl` | 没有输入 impl 时使用 `@Self`，包括自定义常量值内部 | batch-impl: `@Self` is available only on an impl entry (it refers to the input impl's self type) | DSL |
| `const_self_reserved` | 定义自定义 `@Self = ...` | batch-impl: constant name `@Self` is reserved for the input impl's self type; please rename | DSL |
| `generic_family_batch_trait` | `batch_trait!` 里用 `@all_type_params` | batch-impl: `@all_type_params` is supported only by `#[batch_impl]` / `#[batch_impl_only]` (needs a trait definition to read its generic parameters; `batch_trait!` is a function-like macro without one) | DSL |
| `at_num_in_type` | 只有 2 个 fresh 时写 `Box<@5>` | batch-impl: `@5` is out of range — this impl has 2 fresh generics (numbered from 0 in document order; user-written params are addressed by name) | DSL |
| `at_group_in_type` | 类型位置的 `@2_0` | batch-impl: `@2_0` does not match a generated generic — this impl has no group 2 position 0 (groups and positions number from 0); use `@N` for the N-th fresh generic in document order | DSL |
| `at_group_out_of_range` | 同上，另一处位置 | batch-impl: `@2_0` does not match a generated generic — this impl has no group 2 position 0 (groups and positions number from 0); use `@N` for the N-th fresh generic in document order | DSL |
| `at_range_in_type` | 无 fresh 时写 `Vec<@0..=2>` | batch-impl: `@0..=2` out of range — this scope has 0 fresh generics (numbered from 0 in document order) | DSL |
| `at_empty_range_in_angle` | `Box<@2..1>` | batch-impl: empty exclusive range `@2..1` (start not below end) | DSL |
| `at_open_range_bare` | 顶层的 `A@..` | batch-impl: range constant `@..` must name an end point (e.g. `@..u128`, `@..=f64`) | DSL |
| `at_binding_splat` | `Tr<Item = *(A,B)>`；绑定值每个分支只能有一个类型 | batch-impl: this type position requires exactly one type; the pack expands to 2 types | DSL |
| `at_segment_carrier_in_body` | body 里的 `@{...}` 载体 | batch-impl: `@{...}` must hold a position reference (e.g. `@{0}`, `@{1_0..}`, `@{0..=3}`); segment elements are referenced through repeat blocks (`@A`) or an explicit template name (`impl{(A0, @A..)}`), never as `@{...}` | DSL |
| `error_aggregation_codegen` | 多个悬空 `@N` 引用 | batch-impl: `@5` is out of range — this impl has 2 fresh generics (numbered from 0 in document order; user-written params are addressed by name) | DSL |
| `empty_range` | spec 里的空数字区间 | batch-impl: range `3..2` is empty (start not below end); no impls will be generated | DSL |
| `expand_limit` | `(...).2000` | batch-impl: `tuple .2000` expands to 2000 impls (limit 1024); likely exponential/range/Cartesian typo | DSL |
| `bound_gen_over_limit` | bound 生成器乘积 29791 | batch-impl: `materialization` expands to 29791 impls (limit 1024); likely exponential/range/Cartesian typo | DSL |
| `at_trait_inherent_impl` | 在固有 `impl Vec<u8> {}` 上写 `@trait` | batch-impl: `@trait` is not available on an inherent impl (there is no trait to refer to) | DSL |

### 10.4 binding / bound 与函数类型

| fixture | 触发 | 锁定的措辞 | 来源 |
| --- | --- | --- | --- |
| `concrete_binding` | `Assoc<Item = u32>`（纯类型实参） | batch-impl: binding args (`Item = u32`) are only valid on a trait path (`Conv<Item = u32> X`) or in a bound (`T: Iterator<Item = u8>`) — a concrete type's args are a plain type list | DSL |
| `concrete_bound` | `Wrap<u8: Clone>` | batch-impl: bound args (`T: Clone`) are only valid on a trait path, in a generic declaration (`<T: Clone> Foo`) or in a bound — a concrete type's args are a plain type list | DSL |
| `declaration_binding` | `<Item = u8> Target` | batch-impl: an associated-type binding belongs on the trait application — write `Trait<Item = u8> Target`, not `<Item = u8> Target` (a `<>` block declares parameters) | DSL |
| `binding_bound_empty` | `Conv<Item =>` / `Conv<T:>` | batch-impl: binding `Item =` missing a value (write `Item = u32`) | DSL |
| `fn_named_param_missing_type` | `fn(x:)` | batch-impl: named parameter `x:` is missing a type (write `x: u8`) | DSL |
| `fn_sugar_named_param` | `Fn(x: u8)` | batch-impl: the `Fn(…)` trait sugar does not support named parameters (`Fn(x: u8)`) — remove the name (a named parameter is only valid in a `fn(x: u8)` pointer type) | DSL |
| `hrtb_binder_type_param` | `for<u8>` | batch-impl: a `for<…>` binder holds lifetimes (`for<'a>`) — a type or const parameter is declared on the impl, not in the binder | DSL |
| `dyn_bound_missing` | `dyn Send +` | batch-impl: a `+` in a `dyn` bound list needs a bound after it (e.g. `dyn Iterator<Item = u8> + Send`) | DSL |

### 10.5 指令

| fixture | 触发 | 锁定的措辞 | 来源 |
| --- | --- | --- | --- |
| `fill_bad_comma` | `#fill(m,,n)` / `#fill(,m)` 及对应的 delegate/blanket 作用域 | batch-impl: in directive arguments, a comma is in an illegal position (no leading/consecutive commas) | DSL |
| `directive_scope_unknown` | 不存在的选择/排除名，差集删掉它也报错；畸形 delegate 改名 | batch-impl: item `typo` not found in trait `RemovedUnknown` | DSL |
| `minus_bad_target` | `#fill(-1)` | batch-impl: in directive arguments, after `-` expected an identifier or `[...]` list (e.g. `-foo`, `-[a,b]`) | DSL |
| `directive_bad_follow` | `#m` 后面既无参数也无 body | `#m` must be followed by `(args)` or `[args]` + `{body}` (or directly `{body}`) | DSL |
| `single_name_not_found` | `#name` 指向不存在的成员 | batch-impl: item `no_such` not found in trait `T` | DSL |
| `delegate_on_non_fn` | 对常量用 `#delegate` | batch-impl: #delegate only works on methods; `VALUE` in trait `HasConst` is not a method | DSL |
| `delegate_const` | 同上，另一个常量 | batch-impl: #delegate only works on methods; `LIMIT` in trait `ConstApi` is not a method | DSL |
| `delegate_double_rename` | `#delegate(size=a, size=b)` | batch-impl: #delegate method `size` is renamed twice (`size=...` appears more than once); a method can delegate to only one target | DSL |
| `delegate_rename_missing_left` | `#delegate(=foo)` | batch-impl: #delegate rename `X = Y` needs identifiers on both sides (e.g. `#delegate(size = len)`) | DSL |
| `delegate_call_marker` | delegate body 中的 `receiver.#other` / 前缀 `#call(receiver)` | batch-impl: unknown #delegate call marker; use `receiver.#call`<br>batch-impl: #call is a postfix call marker; write `receiver.#call` | DSL |
| `delegate_call_move` | 两次实际执行的调用消费同一个非 `Copy` 方法参数，转发不自动克隆 | use of moved value: `value` | rustc E0382 |
| `delegate_call_nested_item` | 内嵌函数中的标记保留原样，不属于外层 delegate 的作用域 | unexpected token: `#` | rustc |
| `blanket_ptr` | `#blanket(*const T)` | batch-impl: #blanket does not support `*const`/`*mut` wrappers (deref is unsafe, cannot delegate); write #delegate by hand | DSL |
| `blanket_self_return` | blanket 方法返回裸 `Self` | batch-impl: #blanket method `NewT::new` references bare `Self` in a parameter, return type, or generic constraint; delegation cannot equate the wrapper's `Self` with the inner type — write a `#name{...}` body for this wrapper instead | DSL |
| `blanket_self_in_group` | 组里的 `Self` | batch-impl: #blanket method `GroupSelf::f` references bare `Self` in a parameter, return type, or generic constraint; delegation cannot equate the wrapper's `Self` with the inner type — write a `#name{...}` body for this wrapper instead | DSL |
| `blanket_self_constraints` | 方法类型参数 bound 或 where 谓词中的裸 `Self` | batch-impl: #blanket method `InlineBound::read` references bare `Self` in a parameter, return type, or generic constraint; delegation cannot equate the wrapper's `Self` with the inner type — write a `#name{...}` body for this wrapper instead | DSL |
| `blanket_bad_depth` | `#blanket(...:abc)` | batch-impl: after #blanket `:abc` must come a number (e.g. `Box.Arc:2`) | DSL |
| `blanket_bad_empty_depth` | `#blanket(...:)` | batch-impl: after #blanket `:` must come a number (e.g. `Box.Arc:2`) | DSL |
| `blanket_bad_huge_depth` | `#blanket(...:999999)` | batch-impl: #blanket `:999999` is too large (deref depth must be ≤ 128) | DSL |
| `blanket_depth_zero` | `#blanket(@all_methods){Box:0}` | batch-impl: #blanket `:0` is meaningless (deref depth must be ≥ 1) | DSL |
| `blanket_wrapper_empty` | `#blanket(@all_methods){Box,}` | batch-impl: #blanket wrapper list contains an empty element (e.g. `&,Box`); separate elements with `,` | DSL |

### 10.6 形状模板、重复块与变长段

| fixture | 触发 | 锁定的措辞 | 来源 |
| --- | --- | --- | --- |
| `impl_template_dsl_ops` | `impl{...}` 里写 DSL 算子 | batch-impl: the `impl{...}` template is not a standard Rust type (DSL operators are not allowed inside) | DSL |
| `impl_template_range_constant` | 模板里写范围常量 | batch-impl: the `impl{...}` template is not a standard Rust type (DSL operators are not allowed inside) | DSL |
| `impl_shape_mismatch` | 模板与目标形状不匹配 | batch-impl: `impl{...}` template cannot destructure the target type (generic argument shape differs at segment `Rc`) | DSL |
| `impl_shape_fn_qualifiers` | safe 与 unsafe 函数指针类型不同 | batch-impl: `impl{...}` template cannot destructure the target type (function pointer qualifiers differ (unsafe, ABI, lifetimes, variadic or attributes)) | DSL |
| `impl_shape_const_kind` | 已声明 const 槽匹配类型 | batch-impl: `impl{...}` template cannot destructure the target type (declared const parameter `N` needs a const argument (target `u8`)) | DSL |
| `impl_shape_type_to_const` | 类型槽匹配已声明 const | batch-impl: `impl{...}` template cannot destructure the target type (a type argument cannot bind declared const argument `N`) | DSL |
| `impl_shape_literal_conflict` | 同名既须保持字面又须改变 | batch-impl: binding slot `u8` is bound to different subtrees across merged `impl{...}` templates (`u8` vs `u16`) | DSL |
| `impl_shape_lifetime_arg` | 生命周期实参不一致 | batch-impl: `impl{...}` template cannot destructure the target type (generic argument differs (template `'_` vs target `u8`)) | DSL |
| `impl_shape_varseg_duplicate` | 同一个 `A@..` 出现两次 | batch-impl: `impl{...}` template cannot destructure the target type (duplicate variadic segment prefix `A` (each `ident@..` in one template must be unique)) | DSL |
| `impl_shape_varseg_outside_tuple` | 变长段不在元组里 | batch-impl: `impl{...}` template cannot destructure the target type (a variadic segment (`ident@..`) in a generic argument needs a tuple target (`A<(T@..)>` against `A<(P0, P1)>`)) | DSL |
| `impl_shape_varseg_uneven` | 变长段长度不齐 | batch-impl: `impl{...}` template cannot destructure the target type (variadic segments cannot be split evenly: target tuple has 3 elements after 0 fixed, split across 2 segments) | DSL |
| `impl_inconsistent_binding` | 两个模板给 `X` 绑不同子树 | batch-impl: binding slot `X` is bound to different subtrees across merged `impl{...}` templates (`Box < u32 >` vs `Box`) | DSL |
| `impl_shape_repeat_unknown` | `@X` 没有对应段 | batch-impl: repeat block references unknown variadic segment `@X` (the `impl{...}` template declares no `X@..`) | DSL |
| `impl_shape_repeat_unequal` | 各段长度 2 vs 3 | batch-impl: repeat block segments have different lengths (2 vs 3); all referenced segments must be equal-length | DSL |
| `impl_shape_repeat_driver_conflict` | 驱动 `@A` 与内层 `@B` 冲突 | batch-impl: repeat block driver `@A` conflicts with the inner segment reference `@B` (they must be the same) | DSL |
| `impl_shape_repeat_bare_at` | body 里裸写 `@foo` | batch-impl: `@` inside an impl body must start a repeat block `@(...)..` (or `@ident(...)..` with the driving segment declared) | DSL |
| `impl_shape_repeat_cursor_multi` | 多模板下只有游标块 | batch-impl: a cursor-only repeat block needs a driving segment — with several template segments write `@ident(...)..` declaring the driver | DSL |
| `impl_shape_repeat_invalid_switch` | `impl{@2..1}` | batch-impl: invalid fresh-binding switch — the range covers no fresh (`@2..1` / `@2..=1`); write `@N..` / `@N..=M` with `N <= M` | DSL |
| `impl_shape_repeat_invalid_switch_closed` | `impl{@2..=1}` | batch-impl: invalid fresh-binding switch — the range covers no fresh (`@2..1` / `@2..=1`); write `@N..` / `@N..=M` with `N <= M` | DSL |
| `impl_shape_repeat_no_driver` | 无开关的纯游标 body 块 | expected one of `.`, `;`, `?`, `}`, or an operator, found `,` | rustc |
| `repeat_needs_driver` | `u8 { fn n(&self) -> usize { @(A,).. } }` | batch-impl: a repeat block needs a driving segment or a fresh-binding switch (`impl{@0..}`) to determine its length | DSL |

### 10.7 入口与顶层块

| fixture | 触发 | 锁定的措辞 | 来源 |
| --- | --- | --- | --- |
| `implentry_at_num_banned` | impl 入口 spec 无 fresh 时写 `@0` | batch-impl: `@0` is out of range — this impl has 0 fresh generics (numbered from 0 in document order; user-written params are addressed by name) | DSL |
| `implentry_direct_not_type` | 该写类型的位置写了指令 | batch-impl: the direct form takes exactly one type after the generic declaration (e.g. `<T> Box<T>`) | DSL |
| `implentry_hash_banned` | impl 入口上用 `#fill` | batch-impl: `#` directives are not supported on the ItemImpl entry (write the impl body directly) | DSL |
| `top_level_block_not_last` | `{! m!{…}}` 不是最后一个块 | batch-impl: a `{! ...}` top-level block must be the last block | DSL |
| `top_level_manual_not_last` | 手工顶层形式不在最后 | batch-impl: a `{! ...}` top-level block must be the last block | DSL |
| `top_level_without_attach` | 顶层块没有附着类型 | batch-impl: a top-level `{! ...}` block needs an attached type (the spec body is prepended to the macro input) | DSL |
| `top_level_two_blocks` | 一个 spec 里两个 `{! ...}` 块 | batch-impl: at most one top-level `{! ...}` block per spec | DSL |

### 10.8 `where`

| fixture | 触发 | 锁定的措辞 | 来源 |
| --- | --- | --- | --- |
| `where_missing_body` | 输入在裸 `where` 后立即结束，没有谓词 | batch-impl: `where` predicates are missing a code block {...} | DSL |
| `where_not_a_predicate` | `where{ A B }` | batch-impl: a where predicate must be a Rust predicate — write `T: Bound` (a missing `:`, `T Clone`, is the usual cause); a `*(…)` splat is not expanded inside a predicate, so write the types out | DSL |
| `where_splat_bad` | `where{*[A, B]: Clone}` | batch-impl: a splat cannot be a where-predicate subject (`*[A, B]: Trait`) — a `*(…)` list is a parameter position, and a predicate is a constraint, not a list; write the predicates out (`A: Trait, B: Trait`) | DSL |
| `where_empty_exclusive_range` | `where{@2..2: Clone}` | batch-impl: empty exclusive range `@2..2` (start not below end) | DSL |

### 10.9 预览通道

| fixture | 触发 | 锁定的措辞 | 来源 |
| --- | --- | --- | --- |
| `preview_ok` | `batch_preview! { #[batch_impl(usize, isize)] trait Pv {} }` | batch-impl preview: 2 impl(s) generated | channel |
| `preview_miswrite` | 预览体写错 | batch-impl preview: 1 impl(s) generated | channel |
| `preview_pack` | 具体 Vec 包与复用 fresh 的 Pair 包 | batch-impl preview: 1 impl(s) generated | channel |

### 10.10 已知泄漏（措辞由 rustc 给出）

| fixture | 触发 | 锁定的措辞 | 来源 |
| --- | --- | --- | --- |
| `fn_return_reapply` | `fn(A) -> B C` | cannot find type `A` in this scope | rustc |
| `impl_trait_sync_body_negative` | body 里写 `X<>` 但模板不带 `Tr<>` | trait takes 1 generic argument but 0 generic arguments were supplied | rustc |
| `unsafe_non_fn` | 对非 unsafe trait 用 `unsafe` | implementing the trait `T` is not unsafe | rustc |

锁的另一半是 **3 个 `pass` fixture**：`constant_named_type_arg`（只是*名字*叫 `constant` 的类型参数绝不是 `const` 参数）、`tests/ui/pass/basic.rs` 与 `tests/ui/pass/impl_entry_empty_attribute.rs` 必须保持可编译。

## 11. 交叉

各系统并不独立：每一趟跑在前一趟的产物上，而算子共享同一条 spec 链。本章把交叉集中一处；上面各节保留系统内的规则（见 §7.6 与 §8.6）。

### 11.1 趟顺序**就是**组合顺序

`@` 常量 → `<>` 配对 → `#` 指令 → `where` 处理。它买到什么、禁止什么：

| 趟 | 因此后续各趟看到什么 | 例子 |
|---|---|---|
| `@` | 值可以含**扁平** `<...>`，之后才配对 | `@all_type_params` 展开成扁平声明，配对趟把它变成组 |
| `<>` 配对 | 指令参数与谓词看到的 `<...>` 是**组**，绝不是扁平标点 | 两实参的 bound 在谓词里保住自己的逗号 |
| `#` | 指令看到 `@` 产出的结构，其产物进入类型链 | `#fill(@all_methods, -foo)` 收到家族列表 |
| `where` | 最后一趟切分谓词，并把 `impl{...}` 当作边界 | 谓词可以指名 `@` 填过或 `<>` 填过的类型 |

两条值得记住的推论：`@` 是**唯一**跑在配对之前的趟，因此也是唯一能写扁平 `<...>` 的地方；而 `X<>` **同步**不属于这四趟——它是这四趟之后的 Ty 级 codegen 趟（§13.2），所以标记只在 `@`、`#`、`where` 都说完话之后才被填上。

### 11.2 `@` × `<>`

- 常量的值可以含扁平 `<...>`（11.1）；
- 尖括号块里的 `@N` 引用在该块被当作声明或实参表消费**之前**解析——`<@0..>` 声明每个被覆盖的 fresh；
- 值为列表的常量与任何列表一样**分发**，而 splat 是把它留在一个容器里的手段（实测）：`(@u8..=u16,)` 是两个 impl（`(u8,)`、`(u16,)`），而 `(*(@u8..=u16),)` 是一个覆盖两个成员的 impl。`@u8..u16` 是单元素列表 `[u8,]`，不是切片类型 `[u8]`。

### 11.3 `#` × 类型域，以及 × `@`

- 指令参数属于指令域：`,` 列表、`-name` 排除、`@all` 家族与字面 `[a, b]` 列表。写进去的类型域算子**不被解释**——`#fill(@all_methods, -nope)` 解析的是一个排除项，不是 DSL 表达式。即使 `nope` 不在选中集合里，它也必须是 trait 的现有成员；不存在的排除名报错；
- `@all*` 家族与 `@trait` 喂给作用域，这正是"选择属宏元层、动作属指令"的原因；
- 指令的产物是 spec 链里的一个**块**：单组产物可附着到类型也可单独作 spec，而 `#blanket` 的多 token 产物只能单独作 spec（§6.1）。

### 11.4 splat × 其它

- splat 的元素可以是 `@` 常量（11.2），也可以是生成器（`*[].N`）；
- `impl{...}` 模板里的 splat 是 DSL 算子，而模板必须是标准 Rust 类型——被拒（§8.1）；
- **body** 里的 splat 完全不被解释（`a * b` 仍是乘法）；
- 同一个 splat 在声明块里意为"声明"，在实参表里意为"实参"：构造只有一个，由消费者决定（§2）。

### 11.5 `where` × 其它

谓词由 `X<>` 同步、`@N` 引用与 shape 槽位填充，而且是最后被校验的东西。§7.6 列出它与模板、与 blanket 的 `@0` = 目标泛型、以及与 impl 入口的交叉。

### 11.6 `impl{...}` × 其它

模板在同步之后解析、被 `@` 展开进入，也是 `where` 的边界。它的映射改写原型内容，已经选定的矩阵叶子保持为最终目标（§8.3）；交叉规则见 §8.6。

### 11.7 附件 × 入口

| 入口 | `{body}` | `where{...}` | `impl{...}` | `#` 指令 |
|---|---|---|---|---|
| `#[batch_impl]` 挂在 trait 上 | ✓ | ✓ | ✓ | ✓ |
| `#[batch_impl]` 挂在 impl 上（impl 入口） | 手写 impl 的 body 就是来源 | ✓——来源是 impl 自己的 `where` | ✓——该入口**就是**模板 × 矩阵 | ✓，但 spec 里直接写 `#` 除外（`implentry_hash_banned`） |
| `#[batch_impl_only]` | ✓ | ✓ | ✓ | ✓ |
| `batch_trait!` | ✓ | ✓ | ✓ | ✗——它看不到 trait 定义 |

body 的 `X<>` 只有在**开关模板**（`impl{@trait<>}` / `impl{Tr<>}`）下才会同步：没有开关，标记会到达 rustc（E0107）——`impl_trait_sync_body_negative` 锁定的就是这个行为。

## 12. 上限

| 上限 | 值 | 越界时看到什么 |
|---|---|---|
| 单 spec 的 impl 数 | **1024**，`.N` 幂、范围与笛卡尔积共用（`src/ast/op.rs`） | 定向错误，点出乘积与上限："… expands to 2000 impls (limit 1024); likely exponential/range/Cartesian typo"（`expand_limit`、`bound_gen_over_limit`） |
| 嵌套深度 | **128**，组、链、附件、常量值与 delegate body 共用同一个上限（`src/util/mod.rs`） | 组、常量值与 delegate body 报 "nesting depth exceeds 128 levels"（`deep_nesting`、`nested_bracket_too_deep`、`const_value_deep_nesting`、`delegate_call_depth`）；链与附件报 "…exceeds 129 levels (limit 128)"（`chain_too_deep`、`segments_too_deep`、`attach_too_deep`、`impl_attach_too_deep`） |
| 重复块输出 | **65536 token**（`src/codegen/repeat.rs`） | 预算守卫报出跑飞的那个块 |
| `#blanket` deref 深度 | **128** | "`:999999` is too large (deref depth must be ≤ 128)"（`blanket_bad_huge_depth`） |

**任何上限之下都成立的保证**：错误会**替换**掉 impl——绝不会在诊断旁边留一个半成品 impl；宏绝不 panic（proc macro 里 panic 就是编译器 ICE），因此不变量检查改为报定向诊断；没有任何输入会静默产出零个 impl。这些上限针对的是意外爆炸，而不是说正常情况很慢——实测展开开销在 `README.md`。

## 13. 语义：每一趟保证什么

这是 §1.3 那个顺序背后的契约——你可以依赖什么、宏承诺不做什么。模块级地图在 `docs/zh-CN/architecture.md`；本节讲行为。

### 13.1 四趟预处理

| 趟 | 读 | 保证 |
|---|---|---|
| `@` 常量 | 逐字值，递归 | 值可以含**扁平** `<...>`（配对在它之后，因此看得见）；循环/前向引用在定义处被拒，所以展开一定终止 |
| `<>` 配对 | 扁平的 `<` `>` 标点 | 每个 `<...>` 块变成**一个组**；下游解析不再跟踪 `<>` 深度；`->` 的 `>` 永不参与 |
| `#` 指令 | 指令名 + 其参数；delegate body 表达式中的 `.#call` | 指令域独立解析（`,` 列表、`-name`、`@all` 家族）；参数列表里的类型域算子**不被解释**；可识别的 `.#call` 选择完整方法体形式并转发当前调用 |
| `where` | 完整结构 | 谓词按 depth-0 逗号切分；`impl{...}` 模板是谓词区边界 |

**透传**：`ident![...]` 宏体与 `#[...]` 属性内是任意 Rust，四个递归入口一律不进入。

这个顺序不是约定而是编译器强制的：每一趟只能跑在前一趟产出的状态上，"谁先谁后"在调用点无从重新决定。

### 13.2 `X<>` 同步

`Trait<>`（空尖括号）意为"本 spec 的 trait 实参"。同步是对类型结构的一趟遍历，因此它能到达任何类型位置：

| 表面 | 是否同步 |
|---|---|
| `where` 谓词 | ✓ |
| `impl{...}` 模板 | ✓（模板在它**之后**解析——同步前模板里的 `X<>` 不是合法 Rust） |
| impl 泛型 bound 与 `dyn` bound 尾巴 | ✓ |
| 目标类型 | ✓ |
| **body** | 仅在**开关模板**（`impl{@trait<>}` / `impl{Tr<>}`）下：body 同步是显式选择，缺席时是文档化的 rustc E0107，而不是静默重写 |

这个标记**与 ident 无关**——进去的是本 spec 的实参，所以 `Other<>` 会变成 `Other<…本 spec 实参…>`，而没有任何泛型参数的 trait 同步成裸名（`Tr<>` → `Tr`）；元数不匹配由 rustc 报。

### 13.3 fresh 泛型：命名、编号、冲突

- 需要生成参数的构造（`().N`、`*[].N`、`@0..` 声明）在 Ty 里**携带 fresh 声明**直到 codegen 改名；任何内部载体都不会出现在输出里。
- **显示名**是 `P0`、`P1`……按**文档序**——与 `@N` 用的是同一套编号。
- **冲突集**是 impl 已经写下的每一个 ident：spec 的参数、它们的内联 bound、目标类型、trait 实参、继承与手写的 where 谓词、body、属性、关联类型。模板占位符被**排除**（形状映射会把它们改写掉，计入会让可见编号漂移）。
- `@g_i` 用 `(组, 槽)` 寻址——跨数组分发保持稳定；`@N` 是文档序摊平形式；`@N..` 是开区间，越过末尾即为空。

### 13.4 形状模板、变长段与重复块

规则在 §8；这里只保留契约：替换是**子树拼接**（绝不是文本替换），重复块轮次来自匹配到的段——因此同一个槽位的值在它名字出现的每处都一样。

### 13.5 宏绝不做什么

- **没有 panic 路径**：生产代码里没有 `unwrap` / `expect` / `panic!` / `unreachable!` / `debug_assert!` / `assert!`（proc macro 里 panic 就是编译器 ICE）。内部不变量改为报定向诊断——由 clippy deny 家族加源码级守卫测试（`tests/no_panic/`）机器强制。
- **不静默产出空 spec**：一个"零 impl 且无诊断"的输入就是 bug（`-usize` 与列表里以 `-` 开头的元素曾经正是如此）。
- **不泄漏内部名**：只有显示名；悬空 `@N` 在宏内被拦截，绝不落为 rustc 的 E0412。
- **不新增保留符号**：DSL 保留 `@`、`#` 与文档化的算子集；生成名只在 `P0…` 范围内，并已与你写下的一切做过冲突检查。

## 14. 反直觉情形

下面每一条都是这套表面会招来的疑问，配产生它的那条规则。

**为什么 `fn(A) -> Box u8` 是 `Box<u8>`，而 `fn(A) -> u16 u32` 报错？** 返回类型是类型位置，空格照常应用（§3）；把实参应用到原生类型上是 rustc 的 E0109。要拒掉它就会连带破坏 `-> Box u8`。

**为什么 `Tr<T>::Type` 是一个类型，而不是"trait 应用 + 别的"？** ident 之后的 `<...>` 绑到该 ident，`::` 续接同一条路径——整串是**一个元素**，spec 是目标类型，trait 取被标注者。那个拼写既到不了 "`impl Tr for <T>::Type`"，也到不了 "`impl<T> Tr<T> for ::Type`"（§1.2）。

**为什么 `Head . ::path` 行，而 `Head ::path` 不行？** `.` 与空格是元素边界，`::` 是续接。有 trait 头时两者等价，直到目标以 `::` 开头。

**为什么头后面的 `(::T)` 是追加实参，而不是成为目标？** 组是一个**值**（元组或带括号的类型），不是边界；空格把它应用上去。

**为什么 `where` 谓词里拒绝 splat？** 该子句到输出全程 token 级，所以由谓词终检报出。其余每个参数位置列表都会展开（§4）。

**`*(A,B)` 单独作目标与 `(A,B)` 有什么区别？** 独立 splat 为每个元素分别生成 impl；元组形式为 `(A,B)` 生成一个 impl。`u8`、`u16` 这样的不同元素可以使用 splat 形式。E0119 来自生成的 impl 重叠，例如 `*(u8, u8)`，而不是 splat 作为目标本身（§4.6）。

**为什么 `@0..2` 覆盖两个 fresh？** 排他区间在**每个**位置都不含末尾，于是类型路径与 where 谓词路径一致——闭区间写 `@0..=1`。

**为什么两 fresh 的 impl 上 `where{@5..: Clone}` 不报错？** 越过末尾的开区间什么都不贡献——依赖元数的 spec 不该因为短的那个 fresh 少就失败。

**为什么 `<>` 块里的 fresh 生成器报错？** 那个块**就是** impl 的参数表，其 fresh 会被声明却永不被使用（E0392）。把生成器写在类型上：`T.*[].2` 把生成的参数拼进去，`T<()2>` 把它们保持为一个元组实参（两者都实测过）——而那里的普通 splat 是合法的（`<*(A,B)>` → `<A, B>`）。

**为什么 impl 入口的空 spec 列表会原样重发那个块？** 入口是**派生**（每段 spec 派生 0..N 个 impl），所以空列表就是恒等：你写的那个块原样回来。

**为什么 trait 的 `where T: Clone` 并进了参数，而不是 impl 的 where 子句？** 单类型参数谓词属于那个参数，成为它的内联 bound；其余谓词带位置替换逐字通过（§7.2）。

**为什么生成名永不与我的名字冲突？** fresh 显示名是对着 impl 写下的每个 ident 选出来的（13.3）——包括 bound、谓词与 body。

**为什么 body 里的 `X<>` 有时不同步？** body 同步是显式选择：只有开关模板（`impl{@trait<>}` / `impl{Tr<>}`）会打开它，不含开关的模板把 body 的标记留给 rustc（E0107），这是文档化行为（§13.2）。

**为什么 `#[batch_impl(1.5)]` 是错误而不是类型别名？** DSL 里只有整数是类型（`@N` 与幂就是这么数的），因此 float/string/char 字面量照实报出（见 §10.1）。
