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
| 泛型声明 `<…>` | ✓ | ✗ 定向错误（声明的是**参数**；给出 trait 应用的写法） | ✗ 未展开（实测 `<T, *(A,B)>` 原样泄漏） | ✗ 交给 rustc（实测 `<*().3>` → "expected type, found `@`"；ui `decl_generator_splat`） | ✓（`<@0..>` 声明 fresh） | ✓（`A<>` 头部展开） |
| 纯类型实参 `Vec<…>` | ✗ 定向错误 | ✗ 定向错误（ui `concrete_binding` / `concrete_bound`） | ✓ `T<*(A,B)>` → `T<A,B>` | ✓ | ✓ | ✓ |
| 内联 bound `<T: …>` | ✓ | ✓ | ✗ 未展开（实测：rustc 报 raw pointer 错） | ✓（`Fn.().N` 的 fresh 提升到 impl） | ✓ | ✓ |
| `dyn` / `for<'a>` 尾巴 | ✓ | ✓ | ✓ `dyn Tr<*(A,B)>` → `dyn Tr<A,B>`（实测） | ✓ | ✓ | ✓ |
| where 谓词 | ✓ | — | ✗ 由终检报错（见 §7） | — | ✓（`@N` 族） | ✓ |
| 目标类型 | ✗ | ✗ | ✓ | ✓ | ✓ | ✓ |
| `impl{...}` 模板 | — | — | ✗（模板是标准 Rust 类型，DSL 算子被 syn 拒） | ✗ 同上 | ✓（`@trait` / `@` 在 `expand_consts` 展开） | ✓ |
| body | — | — | ✗（不解释，`a * b` 保持乘法） | — | ✓（`@N`；`@{N}` 需 `impl{@{}}` 开关） | — |
| 指令参数 `#fill(…)` | — | — | — | — | ✓（`@all` 家族、`[a,b]` 列表） | — |

指令域与类型域互不进入：`#` 后只有指令名、`@` 家族标记、`,` 分隔的名字、`-[a,b]` 排除项与字面 `[a,b]` 列表；类型域算子写进指令参数不会被解释。

## 3. apply 系统

| 记号 | 语义 | 例子 |
|---|---|---|
| 空格 | 左结合 apply（累加） | `HashMap K V` = `HashMap<K, V>` |
| `.` | 右结合 apply（嵌套） | `&.Box u8` = `&Box<u8>` |
| `[A, B]` | 列表：每个元素生成一个 impl | `[Box, Rc] u8` = `Box<u8>` + `Rc<u8>` |
| `(A)` | 透明组 | `(u8)` = `u8` |
| `[A]` | 切片类型 | `[u8]` |
| `(A, B)` | 元组 | `(u8, u16)` |
| `T^N` / `T^[A,B]` | 幂：分发到每个元素 | `(u8, u16)^2` = 4 个元组 impl |
| `self` | 恒等前缀（矩阵里的裸类型占位） | `[Box, self] u8` = `Box<u8>` + `u8` |
| `&` / `&mut` | 引用 | `& Box<T>` |
| `*const` / `*mut` | 原始指针 | `*const T` |
| `unsafe` | `unsafe.fn(A) -> B` = `unsafe impl`；`unsafe fn(A) -> B` 只是 unsafe fn **类型** | 见教程 §10 |
| `#[...]` | 附着到 impl 的属性 | `#[cfg(...)]` 门控 |
| `!` | 只作 fn 返回类型 | `fn(u8) -> !` |

嵌套类型是原生的（`HashMap<String, Vec<(u8, u16)>>` 直接写直接解析）；`[]` 是**集合**、`()` 是**序列**，`*` 只是保留来源括号的容器语义。

## 4. splat `*`

**语义**：把容器/生成器摊平进外层列表，**只展开一层**。元组是类型、作为单元素保持（`*((a,b),)` = 一个 `(a,b)` impl），数组 / 嵌套 splat / 生成器 / 组摊平。左操作数按来源括号分语义：`*[A,B] T` 分配（集合）、`*(A,B) T` 追加（列表）；右操作数保持整体直到消费（`T.*(A,B)` = `T<*(A,B)>`，仅在 codegen 展开成 `T<A,B>`）。

**组内孤立 splat**：`(*(a,b))` = `( *(a,b) )`、`[*(a,b)]` = `[ *(a,b) ]`——解析为容器、splat 作为**一个元素**保持，渲染时展开成 `(a, b)` / `[a, b]`。

**实测：哪些位置真的展开**

| 位置 | 结果 |
|---|---|
| 泛型实参 / trait 应用实参 `T<*(A,B)>`、`Conv<*(A,B)> X` | ✓ 展开成 `T<A,B>` / `Conv<A,B>` |
| 元组元素 `(u8, *(u16, u32))` | ✓ 展开成 `(u8, u16, u32)` |
| 数组元素 `[*(A), *(B)]` | ✓（spec 列表位置，expand 阶段摊平） |
| `dyn` bound 尾巴 `dyn Tr<*(u8, u16)>` | ✓ 展开成 `dyn Tr<u8, u16>` |
| **泛型声明块** `<T, *(A,B)>` / `<*(A,B)>` | ✗ **未展开**，原样泄漏给 rustc |
| **fn 参数表** `fn(*(u8, u16))` / `fn(u8, *(u16, u32))` | ✗ **未展开**，原样泄漏给 rustc |
| **内联 bound** `<T: Tr<*(u8, u16)>>` | ✗ **未展开**（rustc：`expected mut or const keyword in raw pointer type`） |
| **where 谓词** `where{T: Tr<*(u8, u16)>}` | ✗ 由谓词终检报错（§7），不泄漏给 rustc |

> 上表后三行是**已知缺口**（实测记录，尚未修）：splat 的展开点目前覆盖 Ty 结构（泛型/trait 实参、元组元素、`dyn` 尾巴），而泛型声明块、fn 参数表、内联 bound 走的是另一条路径。教程 §4 的旧列表曾把它们列为合法位置——已按实测更正。

**其它边界**：`*const` / `*mut` 指针不受影响（按后续 token 区分）；裸 `*`（既非 splat 也非指针）定向报错；splat 单独作目标会摊平成重复（`*(A,B)` → E0119），元组 impl 写 `(A,B)`；`*()^N` 把 fresh 元组包回 splat，供载体追加参数（`T^*()^2` = `<A,B>T<A,B>`）。

## 5. `@` 宏元层

`@` 是**仅有的宏元记号**（`#` 只剩指令名）。它是**词法替换**：展开结果进入原管线，不参与任何域内解析，且在四趟预处理里**最先**跑。逐条的展开示例在教程 §6；本节给记号索引、合法性矩阵与边界。

### 5.1 记号索引

| 类别 | 记号 | 展开成 | 细节 |
|---|---|---|---|
| 名字族 | `@u*` `@i*` `@f*` `@num` `@scalar` | 类型**列表** | 教程 §6.1 |
| 范围族 | `@u8..u128` `@i8..i128` `@f32..f64` | **列表**（闭区间连续段） | 任一端点可省；`usize`/`isize` 不在任何范围族里 |
| trait | `@trait` | trait 路径（`batch_trait!` 里是**该段**自己的路径） | 教程 §6 |
| trait 成员族 | `@all_methods` `@all_constants` `@all_types` `@all_required*` `@all_default*` `@all_ref_methods` `@all_value_methods` `@all_static_methods` | `[a,b,c]` **组**，随后进指令参数解析 | 接收者/必需项过滤属于常量本身 |
| 泛型参数族 | `@all_type_params` `@all_const_params` `@all_lifetimes` | 扁平 `<...>` **声明** | const 参数带完整 `const N: usize`；裸名是 E0747 |
| 包装常量 | `@Cow` | `Cow<'_>` + 内在约束谓词 | 仅 `#blanket` |
| 位置引用 | `@N` `@g_i` `@0..=M` `@N..` `@all_fresh` | 一个 fresh 名，或逗号分隔的一串 | §5.3 |
| 自定义常量 | `@name=值;` | 值本身（逐字 token） | 仅 `batch_trait!` 的前导段 |

### 5.2 按入口看合法性

| 记号 | `#[batch_impl]` | `#[batch_impl_only]` | `batch_trait!` | 备注 |
|---|---|---|---|---|
| 名字族 / 范围族 | ✓ | ✓ | ✓ | 纯词法列表 |
| `@trait` | ✓ 本地名 | ✓ 外部路径（`# path::To::Trait:` 前缀） | ✓ **段级**替换 | 唯一按入口改变含义的常量 |
| `@all*` 成员族 | ✓ | ✓ | ✗ 定向错误 | 它们需要 trait 定义 |
| `@all_type_params` / `@all_const_params` / `@all_lifetimes` | ✓ | ✓ | ✗ 定向错误（ui `generic_family_batch_trait`） | 从 trait 自己的参数拷贝 |
| `@Cow` | ✓（仅 `#blanket`） | ✓（仅 `#blanket`） | ✗ | 它是包装打包常量，不是类型别名 |
| `@N` / `@g_i` / `@0..=M` / `@N..` / `@all_fresh` | ✓ | ✓ | ✓ | 由 codegen 解析（`@trait` 更早解析） |
| `@name=值;` | ✗ 定向错误（ui `const_attr_unsupported`） | ✗ 同上 | ✓ | 0.7.2 的属性宏形式已在 0.8.0 回退 |

### 5.3 位置引用

- **编号**：fresh 泛型按**文档序从 0** 编号，编号就是用户可见的显示名（`@0` → `P0`）。用户自己写的参数用它们自己的名字——`@N` 之所以存在，正是因为 fresh 名不是用户写的。
- **`@g_i` 是本原**：组 `g`、槽 `i`（跨数组分发保持稳定）；`@N` 是文档序摊平形式。
- **范围**：`@N..=M` 闭区间，`@N..` 开到最后一个 fresh。在 where 谓词里，一串覆盖会展开成**每个 fresh 一条谓词**（逗号分隔）。
- **越界**：`@N` 越过末尾是定向错误（`at_num_in_type`）；而**开区间**越过末尾（两 fresh 的 impl 上写 `where{@5..: Clone}`）什么都不贡献——是空、不是错（spec 里的闭区间对应 `empty_range`）。
- **`@all_fresh`** 已废弃：写 `@0..`。
- **在 blanket 包装的 where 子句里**，`@0` 指**目标泛型**（blanket 唯一的 fresh）；那里预处理只替换 `@trait`。
- **排他区间已归一**：`@N..M` 在任何位置都排除 `M`（`@0..2` 覆盖 `P0, P1`）。

### 5.4 惰性、循环与定义

`@` 的值是**逐字 token**，在引用处递归展开；值可以是另一个常量（`@a=@b`）或一个 DSL 表达式。**定义处**拒绝：循环（`@a=@a`）、前向引用（`@b` 之前引用 `@b`）、裸范围端点（`@a=@u8` 无 `..`）。常量值内的嵌套同样受 `MAX_NEST_DEPTH` 约束。

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
| `literal_and_range` | `1.5` / `1..x` | **锁的是深度守卫那条消息**——字面量/范围诊断今天没有触发（记为误导性锁定） | DSL |

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
| `decl_generator_splat` | `<*().3> Vec<u8>` | expected type, found `@` | rustc |
| `fn_return_reapply` | `fn(A) -> B C` | cannot find type `A` in this scope（E0425——`-> B` 已填，类型是符号名） | rustc |
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
