# batch-impl 参考手册

**v0.9.7**（2026-08-29）—— 与 `docs/zh-CN/tutorial.md` 同一版本的表面；本手册只描述**当前状态**，历史见 `docs/zh-CN/CHANGELOG.md`。

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

## 2. 位置 × 构造

同一构造在不同位置**合法性不同**，因为门控是**位置**的属性（`parse::generic::ArgsPosition` + `parse::Ctx { trait_name, bound }`），不是列表形状的属性。

| 位置 | bound `T: Clone` | binding `Item = u32` | splat `*(…)` | 生成器 `()^N` | `@` 引用 | `X<>` 同步 |
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

### 3.4 幂 `^N`

`T^N` 对值或列表分发（`(u8, u16)^2` = 4 个元组 impl，`[Box, Rc]^2 u8` = 4 个），单 spec 的 1024 impl 上限（§11）就是用来报出打错的幂的。`*()^N` 把它的 fresh 参数包回 splat，好让后面的操作数把它们追加进去（`T^*()^2` = `<A,B>T<A,B>`）。

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
| 幂的 `^` 拼写 | `*(u8, u16)^2`、`Box^*()^2`、`Box<()^2>` | **被拒**——"unexpected `^` after the type"；`^` 不是 DSL 算子，幂写 `.N` |
| `#` 指令（§6） | 参数来自 spec 的指令 | 指令域解析自己的参数列表；类型域永不进入，反之亦然 |
| `impl{...}` 模板、变长段与重复块（§8） | `impl{(A@..,)}` 配 `@(…@0,)..` | 模板是标准 Rust（里面没有 splat）；变长段与重复块是模板系统自己的机制 |

## 5. `@` 宏元层

### 5.1 规则

`@` 是**仅有的宏元记号**（`#` 只剩指令名）。替换是**词法**的：值以 token 形式被拼接，**引用处不做任何域内解析**——结果进入正常管线，在那里像手写文本一样被解析。它在四趟里**最先**跑（`@` → `<>` 配对 → `#` → `where`），这才让两件事成立：

- 值里可以含**扁平** `<...>`，因为尖括号配对在它之后；
- 值可以是另一个常量（`@a=@b`）或一整个 DSL 表达式，在引用处递归拼接并展开。

### 5.2 按类别的记号

| 类别 | 记号 | 展开成 | 细节 |
|---|---|---|---|
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
| `where{...}` 里的 `@trait` | 会展开——实测 `<T> TrW<T> u8 where{@trait<T>: Sized}` → `where TrW<T>: Sized` |
| `impl{...}` 里的 `@trait` | 会展开——实测 `Box<u8> impl{@trait<u8>}` → `impl TrI for Box<u8>` |
| `@all*` 家族作指令参数 | 指令域自己的输入——实测 `u8 #fill(@all_methods){7}` 填满该 trait 的每个方法 |
| `@Cow` 作 blanket 包装 | `Cow<'_>` + 该包装的约束谓词——实测 `#blanket(@all_methods){@Cow}` → `impl<P0> … for Cow<'_, P0> where P0: Trait, P0: ToOwned + ?Sized, …`（完整形式由 `tests/features/dsl_macro_meta.rs` 锁定） |
| `batch_trait!` 里的 `@trait` | 逐段替换成该段自己的 trait 路径，因此一段可以复用另一段的常量 |

## 6. `#` 指令

### 6.1 形状，以及能挂在哪

`#指令名(作用域){内容}`。`#name{body}` 是 `#fill` 的单成员特例：`#fill([foo]){body}` ≡ `#foo{body}`。

指令的产物形态决定它挂在哪：`#name` / `#fill` / `#delegate` / 开放扩展的 `{...}` 组是**单组**输出，既能附着到类型上（`T {body}`）也能单独作 spec；`#blanket` 是**多 token** 输出（自带泛型、目标与委托），只能单独作 spec。开放扩展自 0.6.7 起**仅限顶层**——`{! m!{...}}` 把 spec 体前置并在顶层发出宏调用；旧的 impl 内形式 `T {m!{...}}` 自 0.7.2 起废弃但仍接受。

### 6.2 作用域文法

| 元素 | 含义 | 被拒的形态（fixture） |
|---|---|---|
| `name` | 一个 trait 成员 | 成员不存在 → `single_name_not_found` |
| `@all` 家族 | 选中的成员集合（§5.1） | `batch_trait!` 里用 `@all*` |
| `[a, b]` | 字面列表 | — |
| `-name` / `-[a, b]` | 从集合里排除 | `-` 后面什么都没有 → `minus_bad_target`；集合被排空 → `minus_empty` |
| `,` | 分隔元素 | 前导/尾随逗号 → `fill_bad_comma` |
| （空） | — | 参数表为空 → `fill_empty_args` |

### 6.3 逐指令参考

| 指令 | 作用域 | 内容 | 边界 |
|---|---|---|---|
| `#name{body}` | 按名字选一个成员——方法、常量或关联类型 | 该成员的实现 | body 必须匹配该成员的形状 |
| `#fill(scope){body}` | 成员集合 | 统一 body，逐个成员从 trait 定义抄签名 | "声明数据、不写重复代码"的核心 |
| `#delegate(scope){target}` | 仅方法 | `fn m(&self, ...) -> R { (target).m(...) }`——跳过 `self`、转发其余参数 | `=new_name` 改名（`X = Y` 两侧都要标识符；重复改名报错）；签名带/返回裸 `Self` 报错并建议 `#name{...}`，而 `Self::Assoc` 返回合法 |
| `#blanket(scope){wrapper list}` | 全体方法 | 每个包装（围绕 fresh `T`）一个完整 impl，按 deref 委托 | 包装可带 `:N`（deref 深度 ≤ 128）；`*const`/`*mut` 包装被拒；`@Cow` 是打包常量 |
| `{! m!{...}}`（开放扩展） | 仅顶层 | 把参数、body 与 trait 定义交给 `m!` | 既非内置也非 trait 成员的名字会展开成**你自己的**宏——所以拼错以 rustc 的 "macro not found" 出现 |

`batch_trait!` **一条都不支持**（它看不到 trait 定义）；每条指令的完整参数语义在 rustdoc（`src/doc/directive_*.md`）。

### 6.4 `# path::To::Trait:` 不是指令

它是 **spec 前缀**（仅 `batch_impl_only`），声明外部 trait 的真实路径：至少含一个 `::`，随后 `@trait` 与所有路径引用都用它（`src/doc/batch_impl_only.md`）。尾部 ident 与被标注的 trait 名不同 → `path_prefix_mismatch`。

## 7. `where`

### 7.1 三种形式

| 形式 | 拼写 | 备注 |
|---|---|---|
| 后缀 | `Trait<A> Target where{P1, P2}` | 附件块——顺序自由 |
| 裸形式 | `Trait<A> Target where P1 { body }` | 谓词直接跟着 body；缺 `{...}` 是 `where_missing_body` |
| 继承 | 被标注 trait 定义上的 `where` | 并入每个 impl（7.2） |

### 7.2 继承是**位置式**的，不是按名字

trait 自己的参数与 spec 的 trait 实参**按位置**配对，这决定了三件事：

- 谓词里的 trait 参数按**位置**跟随，因此**trait 参数改名没问题**：`trait Store<T, K> where T: Clone` 配 `<X, Y> Store<X, Y> usize` 生成 `impl<X: Clone, Y> Store<X, Y> for usize`（由 `features::dsl_where::subst_renamed_generics` 锁定）——0.9 之前那条"改名中断继承"的拒绝已不存在；
- **单类型参数**谓词（`T: Clone`）并进该参数的**内联 bound**；其余谓词带替换逐字通过（`HashMap<T, K>: Send` → `HashMap<X, Y>: Send`）；
- trait 的内联参数 bound 以同样方式继承。

### 7.3 渲染前会被填上的东西

| 标记 | 来自 | 规则 |
|---|---|---|
| `Trait<>` | 本 spec 的 trait 实参 | 同步（§1.3） |
| `@N` / `@g_i` / 区间 | impl 的 fresh 泛型 | §5.3；`@N..` 变成**多条**谓词 |
| `impl{...}` 槽位 | 形状映射 | §8.3 |

### 7.4 终检

所有填充跑完后，谓词会被当作**Rust 谓词**解析，失败由 DSL 报出（§10.8）。会被拒的：漏 `:`（`where{ A B }`）、splat（`(*(A,B)): Trait`、`X: Trait<*(A,B)>`——没有任何阶段展开谓词里的 splat）、裸 splat 主体（`where_splat_bad`）、空排他区间（`where_empty_exclusive_range`）。

## 8. `impl{...}` 形状模板

### 8.1 形态与解析点

模板里是**标准 Rust 类型**（`impl{Container<U>}`）；里面的 DSL 算子被拒（`impl_template_dsl_ops`），且它**只解析一次**——就在 `X<>` 同步之后（同步前 `impl{GenW<>}` 不是合法 Rust）。`@trait` / `@` 在常量阶段进入它，`where_process` 把它当作谓词区边界。

### 8.2 逐位匹配

| 模板 vs 叶子目标类型 | 结果 |
|---|---|
| 该位置的 ident 与目标**相同** | 字面，原样保留 |
| ident **不同** | 槽位，绑定到目标的那棵子树 |
| 多个模板 | 合并成一份映射——同形重复合法，冲突是 `impl_inconsistent_binding` |
| 形状不匹配（元数/种类/结构） | `impl_shape_mismatch`，并指出形状 |

### 8.3 槽位重写什么

替换会到达**目标类型**、**`where` 谓词**与 **body**；槽位是**子树**而不是文本 token（整棵值被拼接）。由 `features::shape_template_advanced::slot_rewrite_reaches_where` 与 `impl_multiple_templates_merge` 锁定。

### 8.4 变长段、重复块与 fresh 开关

| 拼写 | 作用 |
|---|---|
| 模板里的 `A@..` | 标记**变长段**——一个形状族覆盖的元数 |
| body 里的 `@(…@0,)..` | **重复块**：每个被覆盖元素一轮，`@ident` 拼接该轮的子树 |
| `impl{@0..}` | **fresh 绑定开关**：每轮一个 fresh（纯游标块），并启用 `@{N}` 引用 |
| `impl{@{}}` | body 槽开关，在重复块否则会把 `@` 读成块首的地方启用 `@{N}` |

边界（各自在 §10.6 有 fixture）：body 里的裸 `@` 是 `impl_shape_repeat_bare_at`；纯游标块需要驱动——`impl_shape_repeat_cursor_multi`；各段长度不等 `impl_shape_repeat_unequal`；未知段 `impl_shape_repeat_unknown`；驱动冲突 `impl_shape_repeat_driver_conflict`；开关区间覆盖不到 fresh `impl_shape_repeat_invalid_switch`。可运行的例子在教程 §8.4。

## 9. 入口

| 入口 | 形态 | 说明 |
|---|---|---|
| `#[batch_impl]` | 属性宏，挂在 `trait` 定义上 | 重发 trait 定义 + 生成 impl |
| `#[batch_impl]` | 属性宏，挂在 `impl` 块上（**impl 入口**，0.8.0） | 从一个手写 impl × 形状模板批量实例化 |
| `#[batch_impl_only]` | 属性宏，挂在 `trait` 定义上 | 只生成 impl，trait 来自外部（改名前缀 `# path::To::Trait:`） |
| `batch_trait!` | 函数式宏 | 分段 + 自定义 `@name=值;` 常量段；**不支持** `#` 指令 |
| `batch_preprocess_test!` | 测试用 | 只跑预处理、不断言生成物 |
| `batch_preview!` | 诊断通道 | 把展开结果作为 `compile_error!` 文本打印（唯一稳定的终端通道） |

每个入口的完整参数语义在 `src/doc/`（`batch_impl_only.md`、`batch_trait.md`、`batch_preprocess_test.md`、`batch_preview.md`）。

## 10. 诊断目录

所有诊断都是**编译期**错误，指向最接近根源的用户可见 token（宏生成物 fallback 宏调用行），**一条错误、不级联**。措辞由 `tests/ui/` 的 fixture 锁定，`cargo test --test ui` 逐条核对；**每个 fixture 都在下面出现**（漏一个会让守卫测试失败）。

**来源**列说明这条消息是谁写的：**DSL** = 宏自己的用户语言诊断；**rustc** = 已知泄漏（宏把 token 交出去、由 rustc 抱怨）；**macro** = `batch_trait!` 前端自己的解析错误；**channel** = `batch_preview!` 的输出。

### 10.1 类型与 spec 语法

| fixture | 触发 | 锁定的措辞 | 来源 |
|---|---|---|---|
| `array_and_punct` | `[u8; 3; 4]` / `[u8;]` | array length `[T; N]` missing or malformed (write `[u8; 3]`) | DSL |
| `leading_comma` | `,A` | spec list cannot start with `,` | DSL |
| `dangling_operator` | `A.` | missing operand after `.` (e.g. `T.U`) | DSL |
| `leading_operator` | `.A` | missing operand before `.` (e.g. `T.U`) | DSL |
| `num_as_left_operand` | `0.T` | number `0` cannot be a left operand; use it on the right (e.g. `T.0`) | DSL |
| `literal_and_range` | `1.5` / `1..x` | a bare literal in a type position must be an integer (usize)；range 端点必须是整数 | DSL |
| `decl_generator_splat` | `<*().3> Vec<u8>` | a fresh generator cannot be declared here——把生成器写在类型上（如 `T^()^2`） | DSL |
| `semi_in_spec` | 类型后多写 `;` | unexpected `;` after the type | DSL |
| `plus_at_type_start` | `+A` | `+` is not valid at the start of a type (it belongs in a bound) | DSL |
| `star_misuse` | 裸 `*` | `*` must be a splat (`*[...]` / `*(...)`) or a raw pointer (`*const T` / `*mut T`) | DSL |
| `extern_fn_stray_hash` | `extern "C" fn` 后接 `#(x)` | unexpected `#` in a type position | DSL |
| `lifetime_as_operand` | `'a T` | a lifetime cannot be an apply operand (`'a` belongs in bounds like `T: 'a`) | DSL |
| `qualified_tail_dsl_token` | `Foo<T>::Assoc<@0>` | a `::`-tail segment is a plain Rust path — DSL tokens are not allowed | DSL |
| `global_path_no_ident` | 结尾的 `::` | `::` must be followed by a path segment identifier (e.g. `::std::vec::Vec`) | DSL |
| `path_prefix_mismatch` | `# path::Other: Trait` | path prefix `#...Other` has a trailing ident that differs from the trait name | DSL |
| `group_angle_bare` | `(...)` 里的 `<...>` | a generic declaration `<...>` inside `(...)` needs the trailing-comma tuple form | DSL |
| `bare_impl_trait_target` | 目标位置的 `impl Trait` | a bare `impl` in the spec is a shape template — an `impl <trait-object>` target is not | DSL |
| `error_aggregation` | 一个属性里多个坏 spec | number `0` cannot be a left operand（全部错误都报出，不只第一条） | DSL |
| `trait_path_no_ident` | `batch_trait! { 1: ... }` | `batch_trait!` expects an ident as the trait name | macro |
| `only_semicolon` | `batch_trait! { ; }` | `batch_trait!` expects a trait name | macro |
| `missing_colon` | `batch_trait! { Tr ... }` | `batch_trait!` expects ':' to separate the trait name and impl-specs | macro |

### 10.2 深度上限

| fixture | 触发 | 锁定的措辞 | 来源 |
|---|---|---|---|
| `deep_nesting` | 129 层嵌套组 | nesting depth exceeds 128 levels (perhaps an accidental extra bracket) | DSL |
| `nested_bracket_too_deep` | 130 层 `[` 组 | nesting depth exceeds 128 levels | DSL |
| `chain_too_deep` | 129 层运算符链 | operator chain exceeds 129 levels (limit 128); split the chain | DSL |
| `segments_too_deep` | 129 层空格应用链 | space-application chain exceeds 129 levels (limit 128) | DSL |
| `attach_too_deep` | 129 个附件 | space-application chain exceeds 129 levels (limit 128) | DSL |
| `impl_attach_too_deep` | 同样的链走 impl 入口 | space-application chain exceeds 129 levels (limit 128) | DSL |
| `const_value_deep_nesting` | 常量值嵌套 129 层 | nesting depth exceeds 128 levels in a constant value | DSL |

### 10.3 `@` 常量、引用与范围

| fixture | 触发 | 锁定的措辞 | 来源 |
|---|---|---|---|
| `const_unknown` | `@unknown` | unknown @ constant `@unknown`; built-ins: `@u*` `@i*` `@f*` … | DSL |
| `const_cycle` | `@a=@a` | constant `@a` references unknown `@a` (undefined or defined later) | DSL |
| `const_forward` | `@b` 之前引用 `@b` | constant `@a` references unknown `@b` (undefined or defined later) | DSL |
| `const_bare_endpoint` | `@a=@u8`（无 `..`） | constant `@a` references unknown `@u8`——裸范围端点不是常量 | DSL |
| `const_range_bad` | `@u32..u8` | range start is greater than end: `u32..u8` | DSL |
| `const_reserved_all` | `@all = ...` | constant name `@all` is a reserved `@all` selector; please rename | DSL |
| `const_attr_unsupported` | `#[batch_impl]` 上写自定义 `@name=值;` | custom constants are not supported by `#[batch_impl]` / `#[batch_impl_only]` | DSL |
| `generic_family_batch_trait` | `batch_trait!` 里用 `@all_type_params` | `@all_type_params` is supported only by `#[batch_impl]` / `#[batch_impl_only]` | DSL |
| `at_num_in_type` | 只有 2 个 fresh 时写 `Box<@5>` | `@5` is out of range — this impl has 2 fresh generics | DSL |
| `at_group_in_type` | 类型位置的 `@2_0` | `@2_0` does not match a generated generic — this impl has no group 2 position | DSL |
| `at_group_out_of_range` | 同上，另一处位置 | `@2_0` does not match a generated generic — this impl has no group 2 position | DSL |
| `at_range_in_type` | 无 fresh 时写 `Vec<@0..=2>` | `@0..=2` out of range — this scope has 0 fresh generics | DSL |
| `at_empty_range_in_angle` | `Box<@2..1>` | empty exclusive range `@2..1` (start not below end) | DSL |
| `at_open_range_bare` | 顶层的 `A@..` | range constant `@..` must name the family's maximum endpoint (e.g. `@..u128`) | DSL |
| `at_binding_splat` | `Tr<Item = *(A,B)>` | a splat cannot be an associated-type binding value | DSL |
| `at_segment_carrier_in_body` | body 里的 `@{...}` 载体 | `@{...}` must hold a position reference (e.g. `@{0}`, `@{1_0..}`) | DSL |
| `error_aggregation_codegen` | 多个悬空 `@N` 引用 | `@5` is out of range — this impl has 2 fresh generics（全部报出） | DSL |
| `empty_range` | spec 里的空数字区间 | range `3..2` is empty (start not below end); no impls will be generated | DSL |
| `expand_limit` | `(...).2000` | `tuple .2000` expands to 2000 impls (limit 1024) | DSL |
| `bound_gen_over_limit` | bound 生成器乘积 29791 | bound-generator distribution expands to 29791 impls (limit 1024) | DSL |

### 10.4 binding / bound 与函数类型

| fixture | 触发 | 锁定的措辞 | 来源 |
|---|---|---|---|
| `concrete_binding` | `Assoc<Item = u32>`（纯类型实参） | binding args (`Item = u32`) are only valid on a trait path | DSL |
| `concrete_bound` | `Wrap<u8: Clone>` | bound args (`T: Clone`) are only valid on a trait path, in a generic declaration | DSL |
| `declaration_binding` | `<Item = u8> Target` | an associated-type binding belongs on the trait application — write `Trait<Item = u8> Target` | DSL |
| `binding_bound_empty` | `Conv<Item =>` / `Conv<T:>` | binding `Item =` missing a value (write `Item = u32`) | DSL |
| `fn_named_param_missing_type` | `fn(x:)` | named parameter `x:` is missing a type (write `x: u8`) | DSL |
| `fn_sugar_named_param` | `Fn(x: u8)` | the `Fn(…)` trait sugar does not support named parameters | DSL |
| `hrtb_binder_type_param` | `for<u8>` | a `for<…>` binder holds lifetimes (`for<'a>`) — a type parameter is declared on the impl | DSL |
| `dyn_bound_missing` | `dyn Send +` | a `+` in a `dyn` bound list needs a bound after it | DSL |

### 10.5 指令

| fixture | 触发 | 锁定的措辞 | 来源 |
|---|---|---|---|
| `fill_empty_args` | `#fill()` | the directive's argument list cannot be empty | DSL |
| `fill_bad_comma` | `#fill(,a)` | in directive arguments, a comma is in an illegal position | DSL |
| `minus_empty` | `#fill(@all,-)` | directive arguments cannot be empty | DSL |
| `minus_bad_target` | `#fill(-1)` | in directive arguments, after `-` expected an identifier or `[...]` list | DSL |
| `directive_bad_follow` | `#m` 后面既无参数也无 body | `#m` must be followed by `(args)` or `[args]` + `{body}` (or directly `{body}`) | DSL |
| `single_name_not_found` | `#name` 指向不存在的成员 | item `T` not found in trait `no_such` | DSL |
| `delegate_on_non_fn` | 对常量用 `#delegate` | #delegate only works on methods; `HasConst` in trait `VALUE` is not a method | DSL |
| `delegate_const` | 同上，另一个常量 | #delegate only works on methods; `ConstApi` in trait `LIMIT` is not a method | DSL |
| `delegate_double_rename` | `#delegate(size=a, size=b)` | #delegate method `size` is renamed twice | DSL |
| `delegate_rename_missing_left` | `#delegate(=foo)` | #delegate rename `X = Y` needs identifiers on both sides | DSL |
| `blanket_ptr` | `#blanket(*const T)` | #blanket does not support `*const`/`*mut` wrappers | DSL |
| `blanket_self_return` | blanket 方法返回裸 `Self` | #blanket method `NewT::new` takes/returns `Self` | DSL |
| `blanket_self_in_group` | 组里的 `Self` | #blanket method `GroupSelf::f` takes/returns `Self` | DSL |
| `blanket_bad_depth` | `#blanket(...:abc)` | after #blanket `:abc` must come a number (e.g. `Box.Arc:2`) | DSL |
| `blanket_bad_empty_depth` | `#blanket(...:)` | after #blanket `:` must come a number (e.g. `Box.Arc:2`) | DSL |
| `blanket_bad_huge_depth` | `#blanket(...:999999)` | #blanket `:999999` is too large (deref depth must be ≤ 128) | DSL |

### 10.6 形状模板、重复块与变长段

| fixture | 触发 | 锁定的措辞 | 来源 |
|---|---|---|---|
| `impl_template_dsl_ops` | `impl{...}` 里写 DSL 算子 | the `impl{...}` template is not a standard Rust type (DSL operators are not allowed inside) | DSL |
| `impl_template_range_constant` | 模板里写范围常量 | the `impl{...}` template is not a standard Rust type (DSL operators are not allowed inside) | DSL |
| `impl_shape_mismatch` | 模板与目标形状不匹配 | `impl{...}` template cannot destructure the target type (generic argument shape …) | DSL |
| `impl_shape_fn_bound` | 模板里写 `fn(A) -> B` | `impl{...}` template cannot destructure the target type (template `fn(A) -> …`) | DSL |
| `impl_shape_lifetime_arg` | 生命周期实参不一致 | `impl{...}` template cannot destructure the target type (generic argument …) | DSL |
| `impl_shape_varseg_duplicate` | 同一个 `A@..` 出现两次 | `impl{...}` template cannot destructure the target type (duplicate variadic segment …) | DSL |
| `impl_shape_varseg_outside_tuple` | 变长段不在元组里 | `impl{...}` template cannot destructure the target type (a variadic segment …) | DSL |
| `impl_shape_varseg_uneven` | 变长段长度不齐 | `impl{...}` template cannot destructure the target type (variadic segments c…) | DSL |
| `impl_inconsistent_binding` | 两个模板给 `X` 绑不同子树 | binding slot `X` is bound to different subtrees across merged `impl{...}` templates | DSL |
| `impl_shape_repeat_unknown` | `@X` 没有对应段 | repeat block references unknown variadic segment `@X` | DSL |
| `impl_shape_repeat_unequal` | 各段长度 2 vs 3 | repeat block segments have different lengths (2 vs 3) | DSL |
| `impl_shape_repeat_driver_conflict` | 驱动 `@A` 与内层 `@B` 冲突 | repeat block driver `@A` conflicts with the inner segment reference `@B` | DSL |
| `impl_shape_repeat_bare_at` | body 里裸写 `@foo` | `@` inside an impl body must start a repeat block `@(...)..` | DSL |
| `impl_shape_repeat_cursor_multi` | 多模板下只有游标块 | a cursor-only repeat block needs a driving segment | DSL |
| `impl_shape_repeat_invalid_switch` | `impl{@2..1}` | invalid fresh-binding switch — the range covers no fresh | DSL |
| `impl_shape_repeat_invalid_switch_closed` | `impl{@2..=1}` | invalid fresh-binding switch — the range covers no fresh | DSL |
| `impl_shape_repeat_no_driver` | 无开关的纯游标 body 块 | expected one of `.`, `;`, `?`, `}`, or an operator, found `,` | rustc |

### 10.7 入口与顶层块

| fixture | 触发 | 锁定的措辞 | 来源 |
|---|---|---|---|
| `implentry_at_num_banned` | impl 入口 spec 无 fresh 时写 `@0` | `@0` is out of range — this impl has 0 fresh generics | DSL |
| `implentry_direct_not_type` | 该写类型的位置写了指令 | the direct form takes exactly one type after the generic declaration | DSL |
| `implentry_hash_banned` | impl 入口上用 `#fill` | `#` directives are not supported on the ItemImpl entry | DSL |
| `top_level_block_not_last` | `{! m!{…}}` 不是最后一个块 | a `{! ...}` top-level block must be the last block | DSL |
| `top_level_manual_not_last` | 手工顶层形式不在最后 | a `{! ...}` top-level block must be the last block | DSL |
| `top_level_without_attach` | 顶层块没有附着类型 | a top-level `{! ...}` block needs an attached type | DSL |

### 10.8 `where`

| fixture | 触发 | 锁定的措辞 | 来源 |
|---|---|---|---|
| `where_missing_body` | 裸 `where` 后面没有 `{...}` | `where` predicates are missing a code block {...} | DSL |
| `where_not_a_predicate` | `where{ A B }` | a where predicate must be a Rust predicate — write `T: Bound` | DSL |
| `where_splat_bad` | `where{*(A,B): Clone}` | a splat cannot be a where-predicate subject | DSL |
| `where_empty_exclusive_range` | `where{@2..2: Clone}` | empty exclusive range `@2..2` (start not below end) | DSL |

### 10.9 预览通道

| fixture | 触发 | 锁定的措辞 | 来源 |
|---|---|---|---|
| `preview_ok` | `batch_preview! { #[batch_impl(usize, isize)] trait Pv {} }` | batch-impl preview: 2 impl(s) generated | channel |
| `preview_miswrite` | 预览体写错 | batch-impl preview: 1 impl(s) generated | channel |

### 10.10 已知泄漏（措辞由 rustc 给出）

| fixture | 触发 | 锁定的措辞 | 来源 |
|---|---|---|---|
| `fn_return_reapply` | `fn(A) -> B C` | cannot find type `A` in this scope（E0425；返回类型是类型位置，因此 `C` 被应用成 `B` 的实参——换成原生类型名就是 rustc 的 E0109，而 `-> Box u8` = `Box<u8>` 依赖同一次折叠） | rustc |
| `fn_return_reapply` | `fn(A) -> B C` | cannot find type `A` in this scope（E0425；返回类型是类型位置，因此 `C` 被应用成 `B` 的实参——换成原生类型名就是 rustc 的 E0109，而 `-> Box u8` = `Box<u8>` 依赖同一次折叠） | rustc |
| `impl_trait_sync_body_negative` | body 里写 `X<>` 但模板不带 `Tr<>` | trait takes 1 generic argument but 0 generic arguments were supplied（E0107） | rustc |
| `unsafe_non_fn` | 对非 unsafe trait 用 `unsafe` | implementing the trait `T` is not unsafe | rustc |

锁的另一半是 **3 个 `pass` fixture**：`constant_named_type_arg`（只是*名字*叫 `constant` 的类型参数绝不是 `const` 参数）、`tests/ui/pass/basic.rs` 与 `tests/ui/pass/impl_entry_empty_attribute.rs` 必须保持可编译。

## 11. 上限与保证

| 项 | 值 | 出处 |
|---|---|---|
| 单个 spec 的 impl 数上限 | **1024**（`.N` 幂 / 范围 / 笛卡尔积共用） | `src/ast/op.rs`（`MAX_EXPAND`），ui `expand_limit` / `bound_gen_over_limit` |
| 嵌套深度上限 | **128**（组、链、附件、常量值共用 `MAX_NEST_DEPTH`） | `src/util/mod.rs`，ui `deep_nesting` 等 |
| 重复块输出预算 | **65536 token**（`MAX_REPEAT_TOKENS`） | `src/codegen/repeat.rs` |
| 展开开销 | 1024 impl 的矩阵在亚秒级（实测约 0.2 ms/impl） | `src/testing/perf.rs`（`cargo test --lib perf`） |
| fuzz 内存守卫 | 256 MiB（`GUARD_LIMIT`） | `src/testing/mod.rs` |
| 生产代码无 panic | 无 `unwrap`/`expect`/`panic!`/`unreachable!`/`debug_assert!`/`assert!`，不变量违规走诊断 | clippy deny 家族 + `tests/no_panic/main.rs` |
| MSRV / edition | 1.95.0 / edition 2024 | `Cargo.toml` |
| UI 快照平台 | trybuild 措辞在 CI 里只锁 Linux stable（Windows 上跳过） | CI 与 `tests/ui.rs` |
| 发布包 | 只装构建真正读的东西：`README.md` + `docs/tutorial.md` + `docs/reference.md` + `src/doc/*.md`；`docs/zh-CN/`、`docs/dev-changelog.md`、`tests/` 排除 | `Cargo.toml` 的 `exclude` |

## 12. 稳定性

- **语法冻结（0.7.2）**：已有 token 的语义是**最终**的——后续版本只做**加法**（新指令 / 新常量 / 新工具）、诊断精细化、文档打磨；改已有语义必须是**故意的破坏性发布**。
- **`@N` 稳定性承诺**扩展到了整个表面：`@N` 的编号（按文档序）不会变。
- **文档也是表面**：本手册与教程的示例必须真实——读者/评审会逐条核对，任何展开都先量过再写。发现文档与代码不符时，以**实测**为准改文档，并在 changelog 记录。

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

## 14. 上限与失效模式的细节

| 上限 | 值 | 出处 | 越界时看到什么 |
|---|---|---|---|
| 单 spec 的 impl 数 | 1024 | `src/ast/op.rs`（`MAX_EXPAND`） | 定向错误，点出乘积与上限——"likely exponential/range/Cartesian typo" |
| 嵌套深度 | 128 | `src/util/mod.rs`（`MAX_NEST_DEPTH`） | "nesting depth exceeds 128 levels (perhaps an accidental extra bracket)"；组、链、附件与常量值共用同一个计数器 |
| 重复块输出 | 65536 token | `src/codegen/repeat.rs`（`MAX_REPEAT_TOKENS`） | 预算守卫报出跑飞的那个块 |
| `#blanket` deref 深度 | 128 | 同一条深度规则 | "`:999999` is too large (deref depth must be ≤ 128)" |
| fuzz 分配守卫 | 256 MiB | `src/testing/mod.rs`（仅测试） | 让跑飞的分配在 fuzz 期间变成可捕获的 panic，而不是进程 abort |

**任何上限之下都成立的保证**：错误会**替换**掉 impl（绝不会在诊断旁边留一个半成品 impl——旧快照曾这样）；宏绝不 ICE；没有任何输入会静默产出零个 impl。

## 15. 反直觉情形

下面每一条都是这套表面会招来的疑问，配产生它的那条规则。

**为什么 `fn(A) -> Box u8` 是 `Box<u8>`，而 `fn(A) -> u16 u32` 报错？** 返回类型是类型位置，空格照常应用（§3）；把实参应用到原生类型上是 rustc 的 E0109。要拒掉它就会连带破坏 `-> Box u8`。

**为什么 `Tr<T>::Type` 是一个类型，而不是"trait 应用 + 别的"？** ident 之后的 `<...>` 绑到该 ident，`::` 续接同一条路径——整串是**一个元素**，spec 是目标类型，trait 取被标注者。那个拼写既到不了 "`impl Tr for <T>::Type`"，也到不了 "`impl<T> Tr<T> for ::Type`"（§1.2）。

**为什么 `Head . ::path` 行，而 `Head ::path` 不行？** `.` 与空格是元素边界，`::` 是续接。有 trait 头时两者等价，直到目标以 `::` 开头。

**为什么头后面的 `(::T)` 是追加实参，而不是成为目标？** 组是一个**值**（元组或带括号的类型），不是边界；空格把它应用上去。

**为什么 `where` 谓词里拒绝 splat？** 该子句到输出全程 token 级，所以由谓词终检报出。其余每个参数位置列表都会展开（§4）。

**为什么 `*(A,B)` 单独作目标报错（E0119），而 `(A,B)` 可以？** splat 是参数位置列表；单独作目标会摊平成重复 impl。写元组。

**为什么 `@0..2` 覆盖两个 fresh？** 排他区间在**每个**位置都不含末尾，于是类型路径与 where 谓词路径一致——闭区间写 `@0..=1`。

**为什么两 fresh 的 impl 上 `where{@5..: Clone}` 不报错？** 越过末尾的开区间什么都不贡献——依赖元数的 spec 不该因为短的那个 fresh 少就失败。

**为什么 `<>` 块里的 fresh 生成器报错？** 那个块**就是** impl 的参数表，其 fresh 会被声明却永不被使用（E0392）。把生成器写在类型上（`T^()^2`）——注意那里的普通 splat 是合法的（`<*(A,B)>` → `<A, B>`）。

**为什么 impl 入口的空 spec 列表会原样重发那个块？** 入口是**派生**（每段 spec 派生 0..N 个 impl），所以空列表就是恒等：你写的那个块原样回来。

**为什么 trait 的 `where T: Clone` 并进了参数，而不是 impl 的 where 子句？** 单类型参数谓词属于那个参数，成为它的内联 bound；其余谓词带位置替换逐字通过（§7.2）。

**为什么生成名永不与我的名字冲突？** fresh 显示名是对着 impl 写下的每个 ident 选出来的（13.3）——包括 bound、谓词与 body。

**为什么 body 里的 `X<>` 有时不同步？** body 同步是显式选择：只有开关模板（`impl{@trait<>}` / `impl{Tr<>}`）会打开它，不含开关的模板把 body 的标记留给 rustc（E0107），这是文档化行为（§13.2）。

**为什么 `#[batch_impl(1.5)]` 是错误而不是类型别名？** DSL 里只有整数是类型（`@N` 与幂就是这么数的），因此 float/string/char 字面量照实报出（见 §10.1）。

## 16. 记号表

整套表面用到的每个记号，集中一处。

| 记号 | 名称 | 含义 / 合法位置 |
|---|---|---|
| `.` | 右结合 apply | `A.B` = `A<B>`；也是绝对路径目标之前的元素边界（§1.2、§3） |
| （空格） | 左结合 apply | `HashMap K V` = `HashMap<K, V>`；累加实参 |
| `[...]` | 列表 | 集合：每个元素一个 impl（`[Box, Rc] u8`）；作目标时是切片（`[u8]`） |
| `[...; N]` | 数组类型 | `[u8; 3]` |
| `(...)` | 元组 | `(A, B)`——**序列**，在 splat 下是追加 |
| `(A)` | 透明组 | 与 `A` 同类型（但 `(*(a,b))` 是承载一个 splat 的容器） |
| `<>` | 尖括号 | ident 之后是泛型实参；spec 开头是声明块；含 depth-0 `as` 时是限定头（§1.2） |
| `A<>` | 同步标记 | "本 spec 的 trait 实参"——在 where/模板/bound/目标处被填上（§13.2） |
| `^N` / `^[A,B]` | 幂 | 对值或列表分发：`(u8, u16)^2` = 4 个 impl |
| `*(...)` / `*[...]` | splat | 把容器/生成器拼进外层参数列表；只展开一层；左操作数保留来源括号的语义（§4） |
| `*()N` | 生成器 splat | 提升 fresh 声明并拼入 fresh 元组 |
| `@` | 宏元命名空间 | 常量与位置引用；词法解析、第一趟（§5） |
| `#` | 指令命名空间 | `#name` / `#fill` / `#delegate` / `#blanket` / 开放扩展（§6） |
| `;` | spec 分隔符 | 把属性参数切成 spec；只有分隔符不算内容（§1.1） |
| `,` | 列表分隔符 | 列表、元组、实参与指令参数列表 |
| `-name` | 排除项 | 仅指令参数列表 |
| `!` | never 类型 | 作 `fn` 返回类型（`fn(A) -> !`） |
| `&` / `&mut` | 引用前缀 | `& Box<T>` |
| `*const` / `*mut` | 原始指针前缀 | `*const T` |
| `unsafe` | unsafe 标记 | `unsafe.fn(A) -> B` 标记 **impl**；`unsafe fn(A) -> B` 是 fn **类型** |
| `self` | 恒等前缀 | `self T` = `T`；矩阵里的裸类型占位 |
| `#[...]` | 属性 | 附着到生成的 impl；DSL 绝不进入 |
| `{body}` | body 附件 | 实现块（一个块，顺序自由） |
| `where{...}` | 谓词附件 | 带 `@N`、`X<>` 与 shape 槽位的 Rust 谓词（§7） |
| `impl{...}` | 形状模板附件 | 与目标比对的标准 Rust 类型（§8） |
| `@N` | 位置引用 | 第 N 个 fresh 泛型，文档序（`@0` → `P0`） |
| `@g_i` | 组引用 | 生成器组 `g` 的第 `i` 个 fresh；跨分发稳定 |
| `@N..=M` / `@N..` | 区间 | 闭区间 / 开到末尾；`@N..` 越过末尾为空 |
| `@all_fresh` | 已废弃 | 写 `@0..` |
| `@trait` | trait 路径 | 被标注的 trait；含义随入口（本地 / 外部 / 段级） |
| `@u*` `@i*` `@f*` `@num` `@scalar` | 名字族 | 展开成成员列表 |
| `@u8..u128` `@i8..i128` `@f32..f64` | 范围族 | 连续段，任一端点可省 |
| `@all_methods` … `@all_static_methods` | 成员族 | 指令参数用的选中成员集合 |
| `@all_type_params` `@all_const_params` `@all_lifetimes` | 参数族 | 从 trait 拷来的扁平 `<...>` 声明 |
| `@Cow` | 包装常量 | `#blanket` 打包 |
| `@name=值;` | 自定义常量 | 仅 `batch_trait!` 前导段 |
| `#name{body}` | 单成员指令 | 一个 trait 成员的实现 |
| `#fill(scope){body}` | 批量填充指令 | 一个 body、多个签名 |
| `#delegate(scope){target}` | 委托指令 | 生成转发调用 |
| `#blanket(scope){wrappers}` | 覆盖式委托指令 | 每个包装一个完整 impl |
| `{! m!{...}}` | 开放扩展 | 把 spec 体交给你的宏（仅顶层） |
| `# path::To::Trait:` | 外部路径前缀 | 声明外部 trait 的真实路径（`batch_impl_only`） |

