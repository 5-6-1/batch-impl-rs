# batch-impl 参考手册

**v0.9.8**（2026-09-14）—— 与 `docs/zh-CN/tutorial.md` 同一版本的表面；本手册系统性给出各规则系统、它们的交叉与边界情形，§10 逐字引用每条诊断的原文。它只描述**当前状态**，历史见 `docs/zh-CN/CHANGELOG.md`。

**查阅型文档**：完整的表面、合法性矩阵、边界与保证。**学习路径**在 `docs/zh-CN/tutorial.md`（从一行 impl 讲到矩阵组合）——本手册假定你已经见过 DSL 的基本形状，只回答"允许什么 / 不允许什么 / 报什么错 / 上限在哪"。

两条纪律决定了本手册的写法：

- **一条事实只有一个真相源**：教程负责"怎么写 / 为什么"，本手册负责"合法性与边界"，每条 API 的完整参数语义在 rustdoc（`src/doc/*.md`：`batch_impl_only.md`、`batch_trait.md`、`batch_preview.md`、`directive_fill.md`、`directive_delegate.md`、`directive_blanket.md`、`directive_name.md`、`directive_open.md`、`directive_consts.md`）。三处不复制同一句话。
- **每条断言可核对**：文中"实测"指用 `batch_preview!` 或真编译量过（`cargo check` 看 rustc 诊断）；诊断措辞一律由 `tests/ui/` 的 fixture 锁定，fixture 名在本手册 §10 给出。

## 1. spec 文法

### 1.1 属性参数是一串 spec

`#[batch_impl(spec; spec; ...)]`、`batch_trait!` 的分段、以及 impl 入口（`#[batch_impl]` 挂在 `impl` 块上）共用同一套 spec 文法：

| 概念 | 说明 |
|---|---|
| 属性参数 | `;` 分隔的 spec 列表；**分隔符不算内容**——整串为空时（`#[batch_impl]` / `#[batch_impl()]` / `#[batch_impl(;)]`）属性入口原样重发该 item，impl 入口原样发射原块（恒等，0 个 impl） |
| 一个 spec | 一个**类型矩阵**；矩阵的每个格子生成一个 impl |
| spec 形状 | `[<泛型声明>] [trait 应用] 目标类型`，外加任意顺序的附件 |
| 附件 | `{body}`（实现体）、`where{...}`（谓词）、`impl{...}`（Self 形状模板） |

附件是**块**：0.9.0 起它们由空格/`.` 链折叠（0.8.0 的"剥尾随后缀"循环已删），顺序自由，链条深度上限 `MAX_NEST_DEPTH = 128`（`src/util/mod.rs`，ui `attach_too_deep`）。

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

**透传守卫**：`ident![...]` 宏体与 `#[...]` 属性内是任意 Rust，四个递归入口一律不进入（判定收敛在 `scan::bracket_is_passthrough`；0.5.7 曾因缺一处守卫误展开 `#[...]` 里的 `#name`）。

### 1.4 记号一张表

只列没有自己那一节的记号——`@`、`#`、`<>`、splat 与幂由各自的小节讲。

| 记号 | 含义 |
|---|---|
| `.` / 空格 | 同一个 apply 的两种结合性：`.` 嵌套（右结合），空格累加（左结合）；也是绝对路径目标之前的元素边界（§3.2、§1.2） |
| `[...]` / `[A, B]` | 集合：每个元素一个 impl；作类型时是切片或数组（§3.3） |
| `(...)` / `(A)` | 元组 / 透明组——被应用时算**一个**实参（§3.2） |
| `&` `&mut` `*const` `*mut` `unsafe` `self` `#[...]` `!` | 前缀与修饰符，各自作用于紧随其后的块：`self` 是恒等，`unsafe.fn(A) -> B` 标记 impl 而 `unsafe fn(A) -> B` 是 fn 类型，`!` 是返回类型（§3.5） |
| `{body}` / `where{...}` / `impl{...}` | 三种附件块，顺序自由（§7、§8） |
| `;` | 分隔同一个属性参数里的各个 spec；只有分隔符不算内容（§1.1） |
| `,` | 分隔列表、元组、实参与指令参数的元素 |
| `-name` | 排除项，仅指令参数列表（§6.2） |
| `.N` / `()N` | 幂：`T.*().2` 把生成的参数拼进去，`T<()2>` 把它们保持为一个元组实参。`^` **不是**算子——`(u8, u16)^2` 得到的是退休算子消息（§3.4、§10.1） |

## 2. 位置 × 构造

同一构造在不同位置**合法性不同**，因为门控是**位置**的属性（`parse::generic::ArgsPosition` + `parse::Ctx { trait_name, bound }`），不是列表形状的属性。

| 位置 | bound `T: Clone` | binding `Item = u32` | splat `*(…)` | 生成器 `().N` | `@` 引用 | `X<>` 同步 |
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

块是一个原子：路径、组 `(...)`、列表 `[...]`、元组、前缀（`&`、`&mut`、`*const`、`*mut`、`unsafe`、`self`、`#[...]`）、splat（`*(...)` / `*[...]`）、生成器（`().N`）、`@` 常量的展开结果，或某条指令的产物。附件（`{body}`、`where{...}`、`impl{...}`）也是块，可以以任意顺序跟在 spec 之后（§1.1）。

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

`[A, B] T` 把后面的类型分发到每个元素——各生成一个 impl（`[Box, Rc] u8` → `Box<u8>` + `Rc<u8>`）；裸列表作目标时就是同一件事的另一种写法。`(A, B)` 是一个元组值；`(A)` 透明；`[A]` 作类型是切片、`[u8; 3]` 是数组。列表是**集合**、元组是**序列**——这个区别在 splat 操作数下才显出来（§4）。

### 3.4 幂 `.N`

幂写作 `.N`，跟在被重复的那个值后面：`T.N` 把元组或生成器展开成 `N` 个位置的笛卡尔积——`(u8, u16).2` 是 `{u8, u16}` 上的全部有序对，即 4 个 impl；`Frac.*(*@u*).2` 把 `@u*` 列表喂进两个泛型位，得到 36 个（`examples/typeclass.rs` 就是这个拼写；实参形式 `Frac<*(*@u*).2>` 给出同样的 36 个）。单 spec 的 1024 impl 上限（§11）就是用来报出打错的指数的。

`*().N` 把它的 fresh 参数包回 splat，好让后面的操作数把它们追加进去：`T.*().2` 声明两个 fresh 并用在目标里（`impl<P0, P1> … for T<P0, P1>`）。

**`^` 不是算子**：`(u8, u16)^2`、`Box^*()^2`、`Box<()^2>` 一律被拒，报的是 §10.1 逐字引用的退休算子消息（`caret_power_retired`）——一条错误、span 落在这个 `^` 上，并给出可用的 `.N` 拼写。**bound 位置**的 `^` 此前被静默丢弃（`<T: Tr^u8>` 渲染成 `<T: Tr>`），现在同样报出这条消息。更早的文档与 changelog 用 `^` 写幂，请写 `.N`。

### 3.5 `self` 与裸类型占位

`self` 是恒等前缀：`self T` = `T`。在矩阵里它代表裸类型本身（`[Box, self] u8` → `Box<u8>` **和** `u8`），"包装或裸"这一类族就是这么写的。

### 3.6 apply 在哪里停

当头部就是被标注的 trait（或 `@trait`）时，第一个元素是 **trait 应用**、其余是**目标**；有 trait 头时 `.` 与空格等价。ident 之后的 `<...>` 与 `::` 续接路径，而 `.` 与空格是元素边界——绝对路径目标、以及 `Tr<T>::Type` 是一个类型，都由这条规则决定（§1.2）。

### 3.7 边界情形

| 拼写 | 会发生什么 |
|---|---|
| `A.` / `.A` / `,A` | 操作数缺失，定向报错（§10.1） |
| `(A)` 与 `A` | 同一类型；`(*(a,b))` 是把 splat 作为单元素承载的容器 |
| `[A]` 与 `[A, B]` | 切片 vs 两个 impl |
| `Box u8 u16` | `Box<u8, u16>`——两个实参，不是嵌套泛型 |
| `Box Vec u8` | `Box<Vec, u8>`——空格累加；嵌套必须用 `.` |
| `& Box u8` | `&Box<u8>`——前缀吃掉后面那个块 |
| `*(A,B)` 单独作目标 | 重复 impl（E0119）；写 `(A,B)` |
| `HashMap<String, Vec<(u8, u16)>>` 这类嵌套类型 | 直接写、直接解析——不存在"透传"写法 |

## 4. splat `*`

### 4.1 规则

splat 把容器或生成器拼进外层的**参数位置列表**。它在 parse 与 apply 全程保持整体、只在 codegen **展开一次**，因此下游任何环节都看不到"摊平了一半"的实参。展开**只做一层**：

| 写的 | 结果 | 为什么 |
|---|---|---|
| `(u8, *(u16, u32))` | `(u8, u16, u32)` | 元组的元素被拼入 |
| `*((a, b),)` | 一个 `(a, b)` impl | 元组是**类型**，因此作为单元素保持 |
| `Box<*(u8, u16)>` | `Box<u8, u16>` | 泛型实参就是参数列表 |
| `Box<*(*[u8, u16])>` | `Box<u8, u16>` | 嵌套 splat 拼进同一张列表 |
| `[u8, *()]` | 一个 impl（`u8`） | 空 splat 什么都不拼 |

### 4.2 组里的孤立 splat

`(*(a,b))` 解析为**把 splat 作为单个元素**承载的容器——即 `( *(a,b) )`——`[*(a,b)]` 同理；该元素在渲染时展开，因此结果是 `(a, b)` 与 `[a, b]`。这就是容器规则：内容为孤立 splat 的组**就是**那个容器，不是拼接点。

### 4.3 哪个是哪个操作数

- **左操作数——由来源括号决定**：`*[...] T` **分配**，保留集合语义（`*[Box, Rc] u8` → `Box<u8>` + `Rc<u8>`）；`*(...) T` **追加**，保留列表语义（`*(Box, Rc) u8` → 列表 `Box, Rc, u8`，即三个 impl）。
- **右操作数——保持整体**：`T.*(A,B)` 在管线里是 `T<*(A,B)>`、在输出里是 `T<A, B>`（实测：`Box.*(u8, u16)` → `Box<u8, u16>`）。

### 4.4 哪些位置会展开

| 位置 | 结果 |
|---|---|
| 泛型实参 / trait 应用实参 `T<*(A,B)>`、`Conv<*(A,B)> X` | ✓ 展开成 `T<A,B>` / `Conv<A,B>` |
| 元组元素 `(u8, *(u16, u32))` | ✓ 展开成 `(u8, u16, u32)` |
| spec 列表元素 `[u8, *()]`、`[*(u8), *(u16)]` | ✓ 在 expand 阶段摊平（每个存活元素一个 impl） |
| `dyn` bound 尾巴 `dyn Tr<*(u8, u16)>` | ✓ 展开成 `dyn Tr<u8, u16>` |
| 泛型声明块 `<T, *(A,B)>` / `<*(A,B)>` | ✓ 展开成 `<T, A, B>` / `<A, B>`；`*().N` splat 会提升它携带的声明，而那里的**生成器**是定向错误（§10.1） |
| fn 参数表 `fn(*(u8, u16))` / `fn(u8, *(u16, u32))` | ✓ 展开成 `fn(u8, u16)` / `fn(u8, u16, u32)`——`Fn` 家族的 callable（`Fn(*(A,B)) -> C`）是同一张参数表 |
| 内联 bound `<T: Tr<*(u8, u16)>>` | ✓ 展开成 `<T: Tr<u8, u16>>`（从 bound 里提升出来的声明照常落到 impl 上） |
| **`where` 谓词** `where{T: Tr<*(u8, u16)>}` | ✗ 由谓词终检报出（§7），不泄漏给 rustc |

> 最后一行是唯一有意的例外，而且它不是缺口：where 子句从解析到渲染输出全程 token 级，因此由**谓词终检**报出 splat。它上面的三行就是本节所属那次提交修好的——在那之前，它们把 splat 的 token 原样交给 rustc（raw pointer 错、`expected type, found @`）。

### 4.5 边界

| 写的 | 会发生什么 |
|---|---|
| `*const u8` / `*mut u8` | 指针类型：`*` 由后续 token 决定，不当作 splat |
| 裸 `*`（既非 splat 也非指针） | 定向报错（ui `star_misuse`） |
| `*(u8, u16)` 作**目标** | 每个元素一个 impl（`u8`、`u16`）；元素重复会撞车——`*(u8, u8)` 是两个 `impl … for u8`（E0119） |
| `*().2` 作目标 | 每个 fresh 一个 impl（`P0`、`P1`） |
| `Box<*().2>` | 泛型实参承载声明：`impl<P0, P1> … for Box<P0, P1>` |
| `<>` **声明块**里的生成器 | 定向错误——那个块**就是** impl 的参数表（§10.1） |
| `impl{...}` 模板里的 splat | 模板必须是标准 Rust 类型，因此 DSL 算子被拒（§10.6） |
| `where` 谓词里的 splat | 由谓词终检报出（§7） |

### 4.6 交叉

| 与谁 | 拼写 | 实测 |
|---|---|---|
| `@` 常量（§5） | `Box<*(@u*)>` | `Box<u8, u16, u32, u64, u128, usize>`——常量先被拼接，splat 在 codegen 展开 |
| 幂（`.N`） | `*(u8, u16).2` | 八个 impl：四个笛卡尔组合，各自再拼成它的两个元素 |
| 幂的 `^` 拼写 | `*(u8, u16)^2`、`Box^*()^2`、`Box<()^2>` | **被拒**——退休算子消息（§10.1）；`^` 不是 DSL 算子，幂写 `.N` |
| `#` 指令（§6） | 参数来自 spec 的指令 | 指令域解析自己的参数列表；类型域永不进入，反之亦然 |
| `impl{...}` 模板、变长段与重复块（§8） | `impl{(A@..,)}` 配 `@(…@0,)..` | 模板是标准 Rust（里面没有 splat）；变长段与重复块是模板系统自己的机制 |

## 5. `@` 宏元层

### 5.1 规则

`@` 是**仅有的宏元记号**（`#` 只剩指令名）。替换是**词法**的：值以 token 形式被拼接，**引用处不做任何域内解析**——结果进入正常管线，在那里像手写文本一样被解析。它在四趟里**最先**跑（`@` → `<>` 配对 → `#` → `where`），这才让两件事成立：

- 值里可以含**扁平** `<...>`，因为尖括号配对在它之后；
- 值可以是另一个常量（`@a=@b`）或一整个 DSL 表达式，在引用处递归拼接并展开。

### 5.2 按类别的记号

| 类别 | 记号 | 展开成 | 细节 |
| --- | --- | --- | --- |
| 名字族 | `@u*` `@i*` `@f*` `@num` `@scalar` | 类型**列表** | 语言定义的闭集（教程 §6.1） |
| 范围族 | `@u8..u128` `@i8..i128` `@f32..f64` | **列表**——闭区间连续段 | 任一端点可省（`@..u128` = `@u8..u128`）；`usize`/`isize` 不在任何范围族里 |
| trait | `@trait` | trait 路径（`batch_trait!` 里是该段自己的路径） | 唯一随入口改变含义的常量（§5.3） |
| trait 成员族 | `@all_methods` `@all_constants` `@all_types` `@all_required*` `@all_default*` `@all_ref_methods` `@all_value_methods` `@all_static_methods` | `[a,b,c]` **组**，随后进指令参数解析 | 必需/默认与接收者过滤属于常量本身 |
| 泛型参数族 | `@all_type_params` `@all_const_params` `@all_lifetimes` | 从 trait 拷来的扁平 `<...>` **声明** | const 参数带完整 `const N: usize`（裸名是 E0747） |
| 包装常量 | `@Cow` | `Cow<'_>` + 该包装的约束谓词 | 仅 `#blanket` |
| 位置引用 | `@N` `@g_i` `@0..=M` `@N..` `@all_fresh` | 一个 fresh 名，或逗号分隔的一串 | §5.4 |
| 自定义常量 | `@name=值;` | 值本身，逐字 | 仅 `batch_trait!` 的前导段 |

### 5.3 按入口看合法性

| 记号 | `#[batch_impl]` | `#[batch_impl_only]` | `batch_trait!` | 备注 |
|---|---|---|---|---|
| 名字族 / 范围族 | ✓ | ✓ | ✓ | 纯词法列表 |
| `@trait` | ✓ 本地名 | ✓ 外部路径（`# path::To::Trait:` 前缀） | ✓ **逐段**替换 | 唯一随入口改变含义的常量（`src/doc/batch_trait.md`） |
| `@all*` 成员族 | ✓ | ✓ | ✗ 定向错误 | 它们需要 trait 定义 |
| `@all_type_params` / `@all_const_params` / `@all_lifetimes` | ✓ | ✓ | ✗ 定向错误（ui `generic_family_batch_trait`） | 从 trait 自己的参数拷贝 |
| `@Cow` | ✓（仅 `#blanket`） | ✓（仅 `#blanket`） | ✗ | 是包装打包常量，不是类型别名 |
| `@N` / `@g_i` / `@0..=M` / `@N..` / `@all_fresh` | ✓ | ✓ | ✓ | 比 `@trait` 更晚解析，在 codegen |
| `@name=值;` | ✗ 定向错误（ui `const_attr_unsupported`） | ✗ 同上 | ✓ | 属性宏形式已在 0.8.0 回退 |

### 5.4 地址

- **编号与显示名**：fresh 泛型按**文档序**是 `P0`、`P1`……，`@N` 就是这个下标（`@0` → `P0`）。用户自己写的参数用它们自己的名字——`@N` 之所以存在，正是因为 fresh 名不是用户写的。
- **`@g_i` 是本原**：组 `g`、槽 `i`，跨数组分发保持稳定；`@N` 是摊平形式。实测：`().2 where{@0_1: Clone}` → `where P1: Clone`。
- **`@N..=M`** 闭区间，**`@N..`** 开到最后一个 fresh。在 where 谓词里，一串覆盖会变成**每个 fresh 一条谓词**：实测 `().2 where{@1..: Clone}` → `where P1: Clone`。
- **排他区间在每个位置都不含末尾**：实测 `().3 where{@0..2: Clone}` → 三 fresh 的 impl 上得到 `where P0: Clone, P1: Clone`。
- **越过末尾的开区间什么都不贡献**：实测 `().2 where{@5..: Clone}` → 没有谓词、也不报错。依赖元数的 spec 不该因为短的那个情形就失败。
- **`@N` 越过末尾是定向错误**（ui `at_num_in_type`；spec 里闭区间的对应物是 `empty_range`）。
- **blanket 包装的 where 子句里，`@0` 指目标泛型**：实测 `#blanket(own){Box where{@0: Copy}}` → `impl<P0> … for Box<P0> where P0: Trait, P0: Copy`。
- **`@all_fresh` 已废弃**：写 `@0..`。

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

### 6.2 作用域文法

作用域由**指令域**解析，不是类型域（§6.8）：它是一个 `,` 分隔的元素列表。

| 元素 | 含义 | 被拒的形态 |
|---|---|---|
| `name` | 按名字选一个 trait 成员 | 成员不存在 → "item `T` not found in trait `no_such`"（`single_name_not_found`） |
| `@all` 家族 | 选中的成员集合（§5.2）：`@all_methods`、`@all_constants`、`@all_types`、`@all_required*`、`@all_default*`、`@all_ref_methods`、`@all_value_methods`、`@all_static_methods` | 在 `batch_trait!` 里用它（那里没有可选择的 trait 定义） |
| `[a, b]` | 名字的字面列表 | — |
| `-name` / `-[a, b]` | 从集合里排除 | `-` 后面什么都没有 → "after `-` expected an identifier or `[...]` list"（`minus_bad_target`）；集合被排空 → "directive arguments cannot be empty"（`minus_empty`） |
| `,` | 分隔元素 | 前导或尾随逗号 → "a comma is in an illegal position"（`fill_bad_comma`） |
| （什么都没有） | — | 参数表为空 → "the directive's argument list cannot be empty"（`fill_empty_args`） |

### 6.3 `#name{body}`——单个成员

找出名字为 `name` 的**那一个** trait 成员——方法、关联常量或关联类型——用 `body` 填上，body 必须匹配该成员的形状（`usize #to_str{"usize"}`）。它就是短写形式的 `#fill([name]){body}`，一次性场合的惯用选择。

### 6.4 `#fill(scope){body}`——一个 body，多个签名

对每个被选中的成员，**签名从 trait 定义抄来**，`body` 成为它的实现（`#fill([add, add2]){self.0 = self.0.wrapping_add(x as u32)}` 用一个 body 填两个方法）。这是指令系统的核心承诺——声明数据、不写重复代码——也是作用域存在的理由：一个 body，宏把它复制到每个被选中的签名之下。宏**不**对 body 做类型检查；不满足签名的 body 由 rustc 对着生成的 impl 报出。

### 6.5 `#delegate(scope){target}`——生成调用

每个被选中的方法生成一次委托调用：`fn m(&self, ...) -> R { (target).m(...) }`——跳过 `self`、转发其余参数，签名仍来自 trait 定义。`Box.Vec.u32 #delegate(d_len){**self}` 变成 `fn d_len(&self) -> usize { (**self).d_len() }`。

| 形态 | 规则 |
|---|---|
| 作用域 | 仅方法——常量或关联类型是 "`HasConst` in trait `VALUE` is not a method"（`delegate_on_non_fn`、`delegate_const`） |
| target | 一个表达式，被拼进生成的调用（`**self`、字段、构造调用） |
| `foo = call_foo` | 把 trait 的 `foo` 委托给 target 的 `call_foo`：**签名保留 `foo`**，只有调用用另一个名字 |
| 改名缺一侧 | "rename `X = Y` needs identifiers on both sides"（`delegate_rename_missing_left`）；同一方法改两次名是 "method `size` is renamed twice"（`delegate_double_rename`） |

### 6.6 `#blanket(scope){wrapper list}`——每个包装一个完整 impl

`#blanket` 围绕一个 fresh 泛型 `T` 为**每个包装写出一个完整 impl**，把每个被选中的方法按 deref 委托——也就是"每个包装手写一遍 `<T: Trait> wrapper.T #delegate(selected){*…*self}`"的自动化形式。

| 包装列表的元素 | 含义 |
|---|---|
| 一个类型形态 | 为它实现的那个包装，围绕 fresh `T`（`.`/空格链表达嵌套，如 `Box.Arc`） |
| `:N` | 到达内层值的 **deref 深度**（`Box.Arc:2`）——必须是数字，上限 128（`blanket_bad_depth`、`blanket_bad_empty_depth`、`blanket_bad_huge_depth`） |
| `@Cow` | 打包常量：`Cow<'_>` 加上它的内在约束谓词 |

被拒的：`*const`/`*mut` 包装（deref 会不安全，`blanket_ptr`），以及带或返回裸 `Self` 的方法——转发得到的是内层类型而不是包装的 `Self`——报错并建议 `#name{...}`，而 `Self::Assoc` **返回**合法（`blanket_self_return`、`blanket_self_in_group`）。

### 6.7 开放扩展 `{! m!{...}}`

名字既不是内置指令、也不是 trait 成员的 `#name(args){body}`，会展开成**你自己的**同名函数式宏调用，并把参数、body 与 trait 定义交给它。`{! ...}` 块自 0.6.7 起**仅限顶层**——它把 spec 体前置并在顶层发出宏调用——而且必须是最后一个块（`top_level_block_not_last`、`top_level_manual_not_last`、`top_level_without_attach`）；旧的 impl 内形式 `T {m!{...}}`（无 `!`）自 0.7.2 起废弃但仍接受。

值得知道的一点：指令名**没有拼写守卫**，所以拼错的内置指令会静默变成宏调用，并以 rustc 自己的 "macro not found" 出现——先拿 §6.3–§6.6 核对拼写。

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
| 裸形式 | `Trait<A> Target where P1 { body }` | 谓词直接跟着 body；没有那个块就是 "``where`` predicates are missing a code block {...}"（`where_missing_body`） |
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
| 裸 splat 主体——`where{*(A,B): Trait}` | "a splat cannot be a where-predicate subject (`*(A,B): Trait`) — a `*(…)` list is a parameter position, and a predicate is a constraint, not a list; write the predicates out (`A: Trait, B: Trait`)"（`where_splat_bad`） |
| 空排他区间——`where{@2..2: Clone}` | "empty exclusive range `@2..2` (start not below end)"（`where_empty_exclusive_range`） |

### 7.5 边界情形

| 形态 | 会发生什么 |
|---|---|
| `where{T: Clone,}` | 尾随逗号被接受（实测：渲染成 `where T : Clone`） |
| `where{}` | 合法——impl 就是没有 `where` 子句（实测） |
| 谓词后面没有 body 块 | `where_missing_body` |
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

模板与**叶子目标类型**逐位比对：

| 该位置：模板 vs 目标 | 结果 |
|---|---|
| ident 与目标**相同** | 字面——原样保留 |
| ident **不同** | **槽位**，绑定到目标在该位置的那棵子树 |
| 一个 spec 里有多个模板 | 合并成**一份映射**——同形重复合法且冗余，冲突是 `impl_inconsistent_binding` |
| 模板无法解构的形状 | `impl_shape_mismatch` 并指出形状：元数/种类不同、`fn` bound（`impl_shape_fn_bound`）、生命周期实参不同（`impl_shape_lifetime_arg`）、重复的变长段（`impl_shape_varseg_duplicate`）、段不在元组里（`impl_shape_varseg_outside_tuple`）、各段长度不齐（`impl_shape_varseg_uneven`） |

所以模式读作"相同 ⇒ 字面，不同 ⇒ 槽位"：`impl{Container<U>}` 对着 `Vec<i16>` 目标会让 `Container` = `Vec`、`U` = `i16`；而模板里重复目标自己的 ident，就把那个位置钉住。

### 8.3 替换到达哪些面

映射会作用到**目标类型**、**`where` 谓词**与 **body**；槽位是**子树**而不是文本 token——它的值被拼接到名字出现的地方，因此一个槽位可以代表一整个泛型实参（`Vec<i16> impl{SlotBox<T>} where{Vec<T>: Clone}` 渲染成 `where Vec<i16>: Clone`）。这条重写规则对三个面是同一个（由 `features::shape_template_advanced::slot_rewrite_reaches_where` 与 `impl_multiple_templates_merge` 锁定）。

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
| 模板不是标准 Rust 类型 | §8.1 那条消息，一条错误、不级联 |

### 8.6 交叉

- **× `@` 常量**：在常量阶段就被展开进模板，也就是在它被解析**之前**（§13.1）——所以 `impl{@trait<>}`、`@all_type_params` 声明或自定义常量的列表，在模板必须成为合法 Rust 时都已就位。
- **× `X<>` 同步**：同步填上模板里的空尖括号，这也正是解析点排在它后面的原因（§8.1）。真的带 trait 应用的模板（`impl{Tr<>}` / `impl{@trait<>}`）还会打开 **body** 同步；没有这种开关时，body 里的 `X<>` 留给 rustc（E0107）。
- **× `where`**：那一趟把 `impl{...}` 当作谓词区边界，随后形状映射也会替换进谓词（§8.3）——槽位在类型可用的任何地方都合法，谓词内部也是。
- **× impl 入口**：入口（`#[batch_impl]` 挂在 `impl` 块上）就是"形状模板 × 矩阵源"——一个手写原型 impl 加实例化它的 spec（§9）。
- **× apply 系统**：模板是一个块，因此像其它附件一样参与 spec 链，并能与 `{body}` / `where{...}` 以任意顺序附着（§1.1）。

## 9. 入口

六个入口共用 §1 的 spec 文法。先读的对照表在教程 §11；各入口的完整参数语义在 rustdoc（`src/doc/`）。这里只留**规则**形态的东西：

- **`# path::To::Trait:`** 是 spec 前缀而不是指令：它为 `batch_impl_only` 声明外部 trait 的真实路径，要求至少一个 `::`，随后 `@trait` 与所有路径引用都用它。尾部 ident 与被标注 trait 名不同则是 `path_prefix_mismatch`（`src/doc/batch_impl_only.md`）。
- **impl 入口**从你写下的东西派生：手写 impl **自己的** `where` 子句是那里的继承源（§7.6），它的 body 是生成 body 的来源，而 spec 里的 `impl{...}` 模板负责实例化形状族（§8）。
- **`batch_trait!`** 支持分段、自定义 `@name=值;` 定义（§5.5）与**不支持** `#` 指令——它看不到 trait 定义。
- **impl 入口上的空 spec 列表**原样重发那个块：入口是派生，什么都没派生出来就意味着保持原样。

## 10. 诊断目录

所有诊断都是**编译期**错误，指向最接近根源的用户可见 token（宏生成物 fallback 宏调用行），**一条错误、不级联**。措辞由 `tests/ui/` 的 fixture 锁定，`cargo test --test ui` 逐条核对；**每个 fixture 都在下面出现**（漏一个会让守卫测试失败）。

**来源**列说明这条消息是谁写的：**DSL** = 宏自己的用户语言诊断；**rustc** = 已知泄漏（宏把 token 交出去、由 rustc 抱怨）；**macro** = `batch_trait!` 前端自己的解析错误；**channel** = `batch_preview!` 的输出。

### 10.1 类型与 spec 语法

| fixture | 触发 | 锁定的措辞 | 来源 |
| --- | --- | --- | --- |
| `array_and_punct` | `[u8; 3; 4]` / `[u8;]` | batch-impl: array length `[T; N]` missing or malformed (write `[u8; 3]`) | DSL |
| `leading_comma` | `,A` | batch-impl: spec list cannot start with `,` | DSL |
| `dangling_operator` | `A.` | batch-impl: missing operand after `.` (e.g. `T.U`) | DSL |
| `leading_operator` | `.A`，以及前导 `-`（`-usize`、`Vec<u8>, -u16`） | batch-impl: `-` is no longer a type operator (write `A B` or `A.B`; the `-` exclusion only works in directive argument lists like `#fill(@all, -foo)`) | DSL |
| `num_as_left_operand` | `0.T` | batch-impl: number `0` cannot be a left operand; use it on the right (e.g. T.0) | DSL |
| `literal_and_range` | `1.5` / `1..x` | batch-impl: a bare literal in a type position must be an integer (usize); float/string/char literals are not types | DSL |
| `decl_generator_splat` | `<*().3> Vec<u8>` | batch-impl: a fresh generator cannot be declared here — the `<>` block declares the impl's own parameters, so its freshs would be declared and never used; write the generator on the type instead (e.g. `T.*().2`) | DSL |
| `semi_in_spec` | 类型后多写 `;` | batch-impl: unexpected `;` after the type | DSL |
| `plus_at_type_start` | `+A` | batch-impl: `+` is not valid at the start of a type (it belongs in a bound, e.g. `T: Clone + Send`) | DSL |
| `caret_power_retired` | `(u8, u16)^2`、`<T: Tr^u8>` | batch-impl: `^` is no longer a type operator (the power is the `.N` suffix — write `(u8, u16).2` for a tuple and `T.*().2` for a generator) | DSL |
| `star_misuse` | 裸 `*` | batch-impl: `*` must be a splat (`*[...]` / `*(...)`) or a raw pointer (`*const T` / `*mut T`) | DSL |
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

### 10.3 `@` 常量、引用与范围

| fixture | 触发 | 锁定的措辞 | 来源 |
| --- | --- | --- | --- |
| `const_unknown` | `@unknown` | batch-impl: unknown @ constant `@unknown`; built-ins: `@u*` `@i*` `@f*` `@num` `@scalar` and ranges `@u8..u128` `@..u128` `@u16..` | DSL |
| `const_cycle` | `@a=@a` | batch-impl: constant `@a` references unknown `@a` (undefined or defined later; inside a constant definition, only built-in constants or previously defined constants can be referenced) | DSL |
| `const_forward` | `@b` 之前引用 `@b` | batch-impl: constant `@a` references unknown `@b` (undefined or defined later; inside a constant definition, only built-in constants or previously defined constants can be referenced) | DSL |
| `const_bare_endpoint` | `@a=@u8`（无 `..`） | batch-impl: constant `@a` references unknown `@u8` (undefined or defined later; inside a constant definition, only built-in constants or previously defined constants can be referenced) | DSL |
| `const_range_bad` | `@u32..u8` | batch-impl: range start is greater than end: `u32..u8` | DSL |
| `const_reserved_all` | `@all = ...` | batch-impl: constant name `@all` is a reserved `@all` selector; please rename | DSL |
| `const_attr_unsupported` | `#[batch_impl]` 上写自定义 `@name=值;` | batch-impl: custom constants are not supported by `#[batch_impl]` / `#[batch_impl_only]` — write the type matrix directly with `.` / space / `*` instead | DSL |
| `generic_family_batch_trait` | `batch_trait!` 里用 `@all_type_params` | batch-impl: `@all_type_params` is supported only by `#[batch_impl]` / `#[batch_impl_only]` (needs a trait definition to read its generic parameters; `batch_trait!` is a function-like macro without one) | DSL |
| `at_num_in_type` | 只有 2 个 fresh 时写 `Box<@5>` | batch-impl: `@5` is out of range — this impl has 2 fresh generics (numbered from 0 in document order; user-written params are addressed by name) | DSL |
| `at_group_in_type` | 类型位置的 `@2_0` | batch-impl: `@2_0` does not match a generated generic — this impl has no group 2 position 0 (groups and positions number from 0); use `@N` for the N-th fresh generic in document order | DSL |
| `at_group_out_of_range` | 同上，另一处位置 | batch-impl: `@2_0` does not match a generated generic — this impl has no group 2 position 0 (groups and positions number from 0); use `@N` for the N-th fresh generic in document order | DSL |
| `at_range_in_type` | 无 fresh 时写 `Vec<@0..=2>` | batch-impl: `@0..=2` out of range — this scope has 0 fresh generics (numbered from 0 in document order) | DSL |
| `at_empty_range_in_angle` | `Box<@2..1>` | batch-impl: empty exclusive range `@2..1` (start not below end) | DSL |
| `at_open_range_bare` | 顶层的 `A@..` | batch-impl: range constant `@..` must name the family's maximum endpoint (e.g. `@..u128`, `@..f64`) | DSL |
| `at_binding_splat` | `Tr<Item = *(A,B)>` | batch-impl: a splat cannot be an associated-type binding value (`Item = *(A,B)` — bindings take exactly one type; distribute via a spec list like `[Tr<Item=A>, Tr<Item=B>]`) | DSL |
| `at_segment_carrier_in_body` | body 里的 `@{...}` 载体 | batch-impl: `@{...}` must hold a position reference (e.g. `@{0}`, `@{1_0..}`, `@{0..=3}`); segment elements are referenced through repeat blocks (`@A`) or an explicit template name (`impl{(A0, @A..)}`), never as `@{...}` | DSL |
| `error_aggregation_codegen` | 多个悬空 `@N` 引用 | batch-impl: `@5` is out of range — this impl has 2 fresh generics (numbered from 0 in document order; user-written params are addressed by name) | DSL |
| `empty_range` | spec 里的空数字区间 | batch-impl: range `3..2` is empty (start not below end); no impls will be generated | DSL |
| `expand_limit` | `(...).2000` | batch-impl: `tuple .2000` expands to 2000 impls (limit 1024); likely exponential/range/Cartesian typo | DSL |
| `bound_gen_over_limit` | bound 生成器乘积 29791 | batch-impl: bound-generator distribution expands to 29791 impls (limit 1024); reduce the range sizes | DSL |
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
| `fill_empty_args` | `#fill()` | batch-impl: the directive's argument list cannot be empty | DSL |
| `fill_bad_comma` | `#fill(,a)` | batch-impl: in directive arguments, a comma is in an illegal position (no leading/trailing/consecutive commas) | DSL |
| `minus_empty` | `#fill(@all,-)` | batch-impl: directive arguments cannot be empty | DSL |
| `minus_bad_target` | `#fill(-1)` | batch-impl: in directive arguments, after `-` expected an identifier or `[...]` list (e.g. `-foo`, `-[a,b]`) | DSL |
| `directive_bad_follow` | `#m` 后面既无参数也无 body | `#m` must be followed by `(args)` or `[args]` + `{body}` (or directly `{body}`) | DSL |
| `single_name_not_found` | `#name` 指向不存在的成员 | batch-impl: item `T` not found in trait `no_such` | DSL |
| `delegate_on_non_fn` | 对常量用 `#delegate` | batch-impl: #delegate only works on methods; `HasConst` in trait `VALUE` is not a method | DSL |
| `delegate_const` | 同上，另一个常量 | batch-impl: #delegate only works on methods; `ConstApi` in trait `LIMIT` is not a method | DSL |
| `delegate_double_rename` | `#delegate(size=a, size=b)` | batch-impl: #delegate method `size` is renamed twice (`size=...` appears more than once); a method can delegate to only one target | DSL |
| `delegate_rename_missing_left` | `#delegate(=foo)` | batch-impl: #delegate rename `X = Y` needs identifiers on both sides (e.g. `#delegate(size = len)`) | DSL |
| `blanket_ptr` | `#blanket(*const T)` | batch-impl: #blanket does not support `*const`/`*mut` wrappers (deref is unsafe, cannot delegate); write #delegate by hand | DSL |
| `blanket_self_return` | blanket 方法返回裸 `Self` | batch-impl: #blanket method `NewT::new` takes/returns `Self` (bare or `Self::Assoc` projection); blanket delegation forwards the inner type, which cannot match the wrapper's `Self` — write a `#name{...}` body for this wrapper instead | DSL |
| `blanket_self_in_group` | 组里的 `Self` | batch-impl: #blanket method `GroupSelf::f` takes/returns `Self` (bare or `Self::Assoc` projection); blanket delegation forwards the inner type, which cannot match the wrapper's `Self` — write a `#name{...}` body for this wrapper instead | DSL |
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
| `impl_shape_fn_bound` | 模板里写 `fn(A) -> B` | batch-impl: `impl{...}` template cannot destructure the target type (template `fn(A) -> B` does not match target `fn(u8) -> u16`) | DSL |
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
| `where_missing_body` | 裸 `where` 后面没有 `{...}` | batch-impl: `where` predicates are missing a code block {...} | DSL |
| `where_not_a_predicate` | `where{ A B }` | batch-impl: a where predicate must be a Rust predicate — write `T: Bound` (a missing `:`, `T Clone`, is the usual cause); a `*(…)` splat is not expanded inside a predicate, so write the types out | DSL |
| `where_splat_bad` | `where{*(A,B): Clone}` | batch-impl: a splat cannot be a where-predicate subject (`*(A,B): Trait`) — a `*(…)` list is a parameter position, and a predicate is a constraint, not a list; write the predicates out (`A: Trait, B: Trait`) | DSL |
| `where_empty_exclusive_range` | `where{@2..2: Clone}` | batch-impl: empty exclusive range `@2..2` (start not below end) | DSL |

### 10.9 预览通道

| fixture | 触发 | 锁定的措辞 | 来源 |
| --- | --- | --- | --- |
| `preview_ok` | `batch_preview! { #[batch_impl(usize, isize)] trait Pv {} }` | batch-impl preview: 2 impl(s) generated | channel |
| `preview_miswrite` | 预览体写错 | batch-impl preview: 1 impl(s) generated | channel |

### 10.10 已知泄漏（措辞由 rustc 给出）

| fixture | 触发 | 锁定的措辞 | 来源 |
| --- | --- | --- | --- |
| `fn_return_reapply` | `fn(A) -> B C` | cannot find type `A` in this scope | rustc |
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
- 值为列表的常量与任何列表一样**分发**，而 splat 是把它留在一个容器里的手段（实测）：`(@u8..u16,)` 是两个 impl（`(u8,)`、`(u16,)`），而 `(*(@u8..u16),)` 是一个覆盖两个成员的 impl。

### 11.3 `#` × 类型域，以及 × `@`

- 指令参数属于指令域：`,` 列表、`-name` 排除、`@all` 家族与字面 `[a, b]` 列表。写进去的类型域算子**不被解释**——`#fill(@all_methods, -nope)` 解析的是一个排除项，不是 DSL 表达式（实测：排除项谁也没匹配到不算错误）；
- `@all*` 家族与 `@trait` 喂给作用域，这正是"选择属宏元层、动作属指令"的原因；
- 指令的产物是 spec 链里的一个**块**：单组产物可附着到类型也可单独作 spec，而 `#blanket` 的多 token 产物只能单独作 spec（§6.1）。

### 11.4 splat × 其它

- splat 的元素可以是 `@` 常量（11.2），也可以是生成器（`*().N`）；
- `impl{...}` 模板里的 splat 是 DSL 算子，而模板必须是标准 Rust 类型——被拒（§8.1）；
- **body** 里的 splat 完全不被解释（`a * b` 仍是乘法）；
- 同一个 splat 在声明块里意为"声明"，在实参表里意为"实参"：构造只有一个，由消费者决定（§2）。

### 11.5 `where` × 其它

谓词由 `X<>` 同步、`@N` 引用与 shape 槽位填充，而且是最后被校验的东西。§7.6 列出它与模板、与 blanket 的 `@0` = 目标泛型、以及与 impl 入口的交叉。

### 11.6 `impl{...}` × 其它

模板在同步之后解析、被 `@` 展开进入、是 `where` 的边界，并被替换进目标、谓词与 body——§8.6 列出它们。

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
| 嵌套深度 | **128**，组、链、附件与常量值共用同一个计数器（`src/util/mod.rs`） | "nesting depth exceeds 128 levels (perhaps an accidental extra bracket)"（`deep_nesting`、`nested_bracket_too_deep`、`chain_too_deep`、`attach_too_deep`、`const_value_deep_nesting`） |
| 重复块输出 | **65536 token**（`src/codegen/repeat.rs`） | 预算守卫报出跑飞的那个块 |
| `#blanket` deref 深度 | **128** | "`:999999` is too large (deref depth must be ≤ 128)"（`blanket_bad_huge_depth`） |

**任何上限之下都成立的保证**：错误会**替换**掉 impl——绝不会在诊断旁边留一个半成品 impl；宏绝不 panic（proc macro 里 panic 就是编译器 ICE），因此不变量检查改为报定向诊断；没有任何输入会静默产出零个 impl。1024 个 impl 的矩阵远在亚秒级完成（实测约 0.2 ms/impl），所以这些上限针对的是意外爆炸，而不是正常情况慢。

## 13. 语义：每一趟保证什么

这是 §1.3 那个顺序背后的契约——你可以依赖什么、宏承诺不做什么。模块级地图在 `docs/zh-CN/architecture.md`；本节讲行为。

### 13.1 四趟预处理

| 趟 | 读 | 保证 |
|---|---|---|
| `@` 常量 | 逐字值，递归 | 值可以含**扁平** `<...>`（配对在它之后，因此看得见）；循环/前向引用在定义处被拒，所以展开一定终止 |
| `<>` 配对（`angle_collect`） | 扁平的 `<` `>` 标点 | 每个 `<...>` 块变成**一个组**；下游解析不再跟踪 `<>` 深度；`->` 的 `>` 永不参与。**有意破坏性**——只跑一次 |
| `#` 指令 | 指令名 + 其参数 | 指令域独立解析（`,` 列表、`-name`、`@all` 家族）；参数列表里的类型域算子**不被解释** |
| `where` | 完整结构 | 谓词按 depth-0 逗号切分；`impl{...}` 模板是谓词区边界 |

**透传**：`ident![...]` 宏体与 `#[...]` 属性内是任意 Rust。四个递归入口一律不进入，判定只有一处（`scan::bracket_is_passthrough`）——当年漏掉一处守卫，`#[...]` 里的 `#name` 就被误展开了。

这个顺序不是约定而是**类型层**状态机（`preprocess/stream.rs`：`Raw → … → Ready`）：每一趟只能跑在前一趟产出的状态上，"谁先谁后"在调用点无从重新决定。

### 13.2 `X<>` 同步

`Trait<>`（空尖括号）意为"本 spec 的 trait 实参"。同步是对 **Ty 结构**的一趟遍历，因此它能到达任何类型位置：

| 表面 | 是否同步 |
|---|---|
| `where` 谓词 | ✓ |
| `impl{...}` 模板 | ✓（模板在它**之后**解析——同步前模板里的 `X<>` 不是合法 Rust） |
| impl 泛型 bound 与 `dyn` bound 尾巴 | ✓ |
| 目标类型 | ✓（它的 token 快照在同步后**重新取**——同步前的快照会静默丢掉填好的标记） |
| **body** | 仅在**开关模板**（`impl{@trait<>}` / `impl{Tr<>}`）下：body 同步是显式选择，缺席时是文档化的 rustc E0107，而不是静默重写 |

### 13.3 fresh 泛型：命名、编号、冲突

- 需要生成参数的构造（`().N`、`*().N`、`@0..` 声明）在 Ty 里**携带 fresh 声明**直到 codegen 改名；任何内部载体都不会出现在输出里。
- **显示名**是 `P0`、`P1`……按**文档序**——与 `@N` 用的是同一套编号。
- **冲突集**是 impl 已经写下的每一个 ident：spec 的参数、它们的内联 bound、目标类型、trait 实参、继承与手写的 where 谓词、body、属性、关联类型。模板占位符被**排除**（形状映射会把它们改写掉，计入会让可见编号漂移）。
- `@g_i` 用 `(组, 槽)` 寻址——跨数组分发保持稳定；`@N` 是文档序摊平形式；`@N..` 是开区间，越过末尾即为空。

### 13.4 形状模板、变长段与重复块

- 模板是**标准 Rust 类型**（DSL 算子被 syn 拒），与叶子目标类型逐位比对：与目标相同的 ident 是字面，不同的 ident 是槽位、绑定到那棵子树。
- 替换到达**目标、`where` 谓词与 body**——槽位是**子树**（按值拼接），不是文本替换。
- **变长段**（`A@..`）标记模板中变化的那一位；body 的 `@(…@0,)..` 重复块每个被覆盖元素跑一轮、拼接该元素的子树。**fresh 绑定开关**（`impl{@0..}`）让纯游标块每轮绑一个 fresh；`impl{@{}}` 在 `@` 否则会开启块的地方启用 `@{N}` 引用。

### 13.5 宏绝不做什么

- **没有 panic 路径**：生产代码里没有 `unwrap` / `expect` / `panic!` / `unreachable!` / `debug_assert!` / `assert!`（proc macro 里 panic 就是编译器 ICE）。内部不变量改为报定向诊断——由 clippy deny 家族加源码级守卫测试（`tests/no_panic/`）机器强制。
- **不静默产出空 spec**：一个"零 impl 且无诊断"的输入就是 bug（`+A` 当年正是如此）。
- **不泄漏内部名**：只有显示名；悬空 `@N` 在宏内被拦截，绝不落为 rustc 的 E0412。
- **不新增保留符号**：DSL 保留 `@`、`#` 与文档化的算子集；生成名只在 `P0…` 范围内，并已与你写下的一切做过冲突检查。

## 14. 反直觉情形

下面每一条都是这套表面会招来的疑问，配产生它的那条规则。

**为什么 `fn(A) -> Box u8` 是 `Box<u8>`，而 `fn(A) -> u16 u32` 报错？** 返回类型是类型位置，空格照常应用（§3）；把实参应用到原生类型上是 rustc 的 E0109。要拒掉它就会连带破坏 `-> Box u8`。

**为什么 `Tr<T>::Type` 是一个类型，而不是"trait 应用 + 别的"？** ident 之后的 `<...>` 绑到该 ident，`::` 续接同一条路径——整串是**一个元素**，spec 是目标类型，trait 取被标注者。那个拼写既到不了 "`impl Tr for <T>::Type`"，也到不了 "`impl<T> Tr<T> for ::Type`"（§1.2）。

**为什么 `Head . ::path` 行，而 `Head ::path` 不行？** `.` 与空格是元素边界，`::` 是续接。有 trait 头时两者等价，直到目标以 `::` 开头。

**为什么头后面的 `(::T)` 是追加实参，而不是成为目标？** 组是一个**值**（元组或带括号的类型），不是边界；空格把它应用上去。

**为什么 `where` 谓词里拒绝 splat？** 该子句到输出全程 token 级，所以由谓词终检报出。其余每个参数位置列表都会展开（§4）。

**为什么 `*(A,B)` 单独作目标报错（E0119），而 `(A,B)` 可以？** splat 是参数位置列表；单独作目标会摊平成重复 impl。写元组。

**为什么 `@0..2` 覆盖两个 fresh？** 排他区间在**每个**位置都不含末尾，于是类型路径与 where 谓词路径一致——闭区间写 `@0..=1`。

**为什么两 fresh 的 impl 上 `where{@5..: Clone}` 不报错？** 越过末尾的开区间什么都不贡献——依赖元数的 spec 不该因为短的那个 fresh 少就失败。

**为什么 `<>` 块里的 fresh 生成器报错？** 那个块**就是** impl 的参数表，其 fresh 会被声明却永不被使用（E0392）。把生成器写在类型上：`T.*().2` 把生成的参数拼进去，`T<()2>` 把它们保持为一个元组实参（两者都实测过）——而那里的普通 splat 是合法的（`<*(A,B)>` → `<A, B>`）。

**为什么 impl 入口的空 spec 列表会原样重发那个块？** 入口是**派生**（每段 spec 派生 0..N 个 impl），所以空列表就是恒等：你写的那个块原样回来。

**为什么 trait 的 `where T: Clone` 并进了参数，而不是 impl 的 where 子句？** 单类型参数谓词属于那个参数，成为它的内联 bound；其余谓词带位置替换逐字通过（§7.2）。

**为什么生成名永不与我的名字冲突？** fresh 显示名是对着 impl 写下的每个 ident 选出来的（13.3）——包括 bound、谓词与 body。

**为什么 body 里的 `X<>` 有时不同步？** body 同步是显式选择：只有开关模板（`impl{@trait<>}` / `impl{Tr<>}`）会打开它，不含开关的模板把 body 的标记留给 rustc（E0107），这是文档化行为（§13.2）。

**为什么 `#[batch_impl(1.5)]` 是错误而不是类型别名？** DSL 里只有整数是类型（`@N` 与幂就是这么数的），因此 float/string/char 字面量照实报出（见 §10.1）。

