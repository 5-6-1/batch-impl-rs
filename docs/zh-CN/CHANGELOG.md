# Changelog（用户）

> 用户可见的功能与行为变化；内部实现细节见 `docs/dev-changelog.md`。

## 0.6.6 (2026-08-06)

### `(T)^N` 分组剥离语义 + 数字渲染无后缀

- **破坏性变更**：`(T)^N` 此前（0.2.0 起）生成长度 N 的重复元组 `(T, T, ...)`；
  现改为剥离分组等价 `T^N`（普通类型 `^N` 是 const 泛型实参：`(W)^2 = W<2>`，
  其中 `W` 为带 const 泛型的类型）。依赖 `(T)^N` 生成元组的升级用户须改用
  `(T,)^N`；
- `(<T>)` 是错误语法（`(` 后 `<` 不是合法类型）；
- 数字/范围渲染不带 `usize` 后缀（`W<2>` 而非 `W<2usize>`、`[u8; 3]` 而非
  `[u8; 3usize]`）。

### 输入校验护栏（评测员发现）

- `expand_consts` 深度守卫 128 层（超深 `[[[` 嵌套不再栈溢出）；
- `#blanket` `:N` 上限 128（`Box:999999` 不再栈溢出）；
- batch_trait! 常量定义拦截 `@all_*` 保留名（定义处报错）；
- `#blanket` `Box:`（冒号后空）在 DSL 层报错。

### `#delegate` 支持参数模式

- 可作表达式的参数模式（如 `(a, b)`）保留签名，委托调用直接以模式 token
  重建转发；`ref x`、守卫、`_` 及其嵌套形式（`(ref x, ref y)`）自动命名
  （`arg0`…）转发。

### 输入校验补全

- batch_trait! 常量定义拦截裸 `@all`；
- 常量值引用校验（check_value_refs）补 128 层深度守卫；
- 深度守卫前移（Group 递归前拦截）+ 类型注解模式（`x: u32`）委托回退命名；
- `#blanket` 包装支持 `@0` 位置标记：带 `@0` 时 T 可放任意位置
  （`(u32, @0)` → `(u32, T)`），不带则 `部分^T` 末尾附加；
- 新增 6 个空占位宏（batch_impl_delegate / fill / blanket / name / open /
  consts）作为指令文档入口——纯 doc 符号，展开为空，误调用无害。

## 0.6.5 (2026-08-06)


### 指令参数方括号写法：`#cmd[args]{body}`

- 指令参数支持 `(args)` 或 `[args]` 等价写法（如 `#fill[@all_methods]{0}`）——
  方括号在参数本身含括号时更清晰；错误消息与教程同步更新。

### 修复：宏调用 passthrough 洞

- `ident!(...)` / `foo!()` 的 `()` 参数组此前无条件进入递归——内部 `@` 常量被
  替换、`<` 被错误配对成角度组；此前只有 `[]` 组有 `!`/`#` passthrough 守卫。
  现在 `()` 组共享守卫（宏调用原样透传；`#name(...)` 指令参数与 DSL 元组仍进入）。

### 行为收紧：裸范围端点引用在定义处报错

- `@a=@u8`（无 `..` 的端点）此前通过 `check_value_refs`、使用处才炸；现在
  定义处直接报错（ui fixture `const_bare_endpoint` 锁定）。

### blanket `@0` / `@N` 统一到 codegen 解析

- blanket 包装 where 的 `@0`/`@N` 原样保留进 spec，由 `resolve_where_at` 与
  普通 where 谓词统一解析（blanket 的 fresh 泛型是唯一 fresh，`@0` 索引到它）；
  预处理只替换 `@trait`。行为等价、架构统一——"`@N` 是唯一 codegen 记号"对
  blanket 包装 where 也成立。

## 0.6.4 (2026-08-05)

### Apply trait 恢复：`apply` 右分发默认实现（span 兼容）

- span 改造时 `trait Apply` 只剩 `apply_help`（右分发被挪到 `TyKind::apply`
  普通方法）——trait 名与主方法名不一致；恢复之前设计：
  - `trait Apply: Clone + Into<TyKind>`——`apply(self, o, span)` 默认实现
    （右操作数结构分发，从 `TyKind::apply` 平移）+ `apply_help` 抽象钩子；
  - `impl Apply for TyKind`（覆写 `is_type_param` + 转发子类型）；
    子类型 `apply_help` 改普通方法（`impl X`，`pub(crate)`）——不再实现
    trait（默认 apply 的 `Ty::new(span, self)` 需要 Self: Into<TyKind>，
    子类型不满足）；
  - `is_type_param()` 默认方法（TyKind 覆写）替代 `matches!(self, ...)`
    ——泛型 Self 无法 match TyKind 变体；
- span 贯穿不变：`Ty::apply` 取 span → `kind.apply(o, span)`（trait 默认，
  每个构造 `Ty::new(span, ...)` 用左操作数 span，`o.span` 仅 fallthrough）；
- 测试全绿（分离声明顺序、数组/范围/泛型外提均回归）。

### `@trait` 提前展开（常量阶段/段级），`@N` 成为唯一 codegen 记号

- 问题：`where{...}` 是 Brace 组，`expand_consts` 原先不进入（body 的 `@` 是
  pattern 语法）——where 谓词里的 `@trait`/`@N` 都残留到 codegen 的
  `resolve_where_at`；作者指出 `@trait` 不该留到 codegen（只有 `@N` 需要
  impl 泛型列表）；
- 修复三处：
  - `expand_consts` 识别 `where` Ident + Brace 组（DSL 结构非 body）→ 进入展开
    `@trait`（batch_impl 用 trait 路径）；`@N`（`@` + Literal）在
    `try_expand_at` 返回 None 保留（不再误报"must be followed by a name"）；
  - `replace_segment_trait`（batch_trait! 段级）递归进组——where{...} 谓词里的
    `@trait` 也能段级替换；
  - `resolve_where_at` 删除 `@trait` 分支——现在只处理 `@N`（签名去掉
    trait_name 参数），架构上"`@N` 是唯一 codegen 解析的记号"成立；
- 验证：batch_impl `where{T: @trait<T>}`（B1）、batch_trait! 段级 where 组内
  `@trait`（探针）都提前展开；`where{@0: Clone}` 纯 fresh 场景回归全绿。

### `@N` 位置引用语义修正：只索引 fresh 泛型

- `@N` 现在指 where 谓词内**第 N 个宏生成的 fresh 泛型**（`_Param_{N}_BatchGen_`
  形式）——用户泛型（`<T>` 等）**不参与 @N 索引**，直接写名字（`where{T: Default}`）；
- 与 blanket 包装谓词的 `@0`（= 目标泛型 fresh T）自然统一：blanket 只有一个
  fresh，`@0` 恰好是"第 0 个 fresh"，不再是特例规则；
- 破坏点：`<T> ... where{@0: Default}` 曾指用户泛型 T——改为 `where{T: Default}`
  （更自然）；越界报错更新（"impl has N fresh generics"）；
- 作者初衷：`@N` 本意就是 `_Param_N_BatchGen_` 的直接映射——fresh 编号是全局
  计数器、与最终位置无关（多 fresh 源/用户泛型混排时错位），故用"第 N 个 fresh"
  加固：位置可数、与编号无关、含用户泛型场景的纯粹性。

### 泛型参数族：`@all_type_params` / `@all_const_params` / `@all_lifetimes`

- 泛型声明照抄 trait 形参：类型参数只名字（`@all_type_params` → `<T, U>`）、
  const 完整声明（`@all_const_params` → `<const N: usize>`）、生命周期原样
  （`@all_lifetimes` → `<'a>`）；bound 由既有同名继承自动补；
- 用法：`#[batch_impl(@all_type_params GenT<T> Vec<T>)]`——声明与 trait 同步，
  改 trait 形参不必改宏；
- 组合（如 `@all_lifetimes @all_type_params`）保持生命周期在前——顺带修复
  了 DSL 分离泛型声明的顺序 bug（`<'a> <T> X` 曾生成 `<T, 'a>`）；
- batch_impl/batch_impl_only 专属（需要 trait_def）；trait 无该类参数时报错。

### `@` 常量名字族改名：`@uint`/`@int`/`@float` → `@u*`/`@i*`/`@f*`

- 名字族符号与范围族统一：`u`/`i`/`f` = 族、`*` = 通配全集——`@u*` 与
  `@u8..u128` 讲的是同一族（原 `uint` 与 `u` 符号不一致是概念裂缝）；
- 语义不变：`@u*` = `[u8, u16, u32, u64, u128, usize]`（含 usize），
  `@i*` = `[i8..isize]`，`@f*` = `[f32, f64]`；`@num`/`@scalar` 不变
  （`@num` = `@u* + @i* + @f*`）；
- **破坏性变更**：`@uint`/`@int`/`@float` 已删除（错误消息提示新名）；
- 实现：`builtin_named` 表 `u*`/`i*`/`f*` 通配（Ident + `*`，消费 3 token）；
  `check_value_refs` 同步识别通配（值内 `@u*` 引用）；ui 快照重生成。
## 0.6.3 (2026-08-05)

### 修正文档

- README（中文 + 英文）头部示例：`()^4` 的展开注释错误——`()^N` 是**单个** N 元组
  （`()^4` → 单个 `(A, B, C, D)`），原注释误写为 4 个不同长度的 impl；长度范围
  应使用 `()^1..=4`。仅修正注释，无行为变化。
## 0.6.2 (2026-08-05)

### 基于 span 的诊断

- 每个 `Ty` 节点携带源 `Span`（`enum Ty` → `struct Ty { span, kind: TyKind }`）；
  `Ty::apply` 取节点自身的 span 并在组合子输出中贯穿——`apply` 内产生的错误
  指向左操作数的位置；
- `compile_error_str` / `compile_err_at!` 接受显式 span；parse、常量、指令、
  blanket、apply 的错误全部接到肇事 token 的 span（`^` 缺操作数现在指向 `^`
  本身，而非整个宏调用）；
- 平台限制（rustc 行为）：属性宏输入的顶层 token 携带精确 span，但组内 token
  退化为 call-site span，且以 `Err` 返回的错误总是显示在宏调用行——真正显示
  精确位置的是 `Ty::Error`（Ok 输出）路径上的 parse/apply 错误；
- `compile_error!` 只把关键字标识符盖上目标 span、其余保持 call-site——若全
  token 都带 span，rustc 会把错误当作 item 位置的用户代码
  （"macros that expand to items must be delimited..."）。

### `#blanket` 静态方法委托

- `#blanket` 现在把无 receiver 的方法（静态方法 / `@all_static_methods` /
  `@all_methods`）经 blanket 泛型 `t` 转发——`fn make() -> u8 { t::make() }`，
  而非 deref 链委托体 `(**self).make()`（静态方法没有 `self`，E0424）；
- 直接调用、嵌套包装（`Box<Box<u8>>`）、参数转发都能经 `t: Trait` bound 到达
  底层 impl——与 assoc item 投影同一转发语义；
- 哲学统一：实例方法经 deref 转发、静态方法经 bound 转发，都是转发，不特判。

### 按 receiver 种类的 `@all` 过滤

- 新增 `@all` 族标记按 receiver 种类过滤 trait 方法：
  `@all_ref_methods`（`&self` / `&mut self`）、`@all_value_methods`
  （`self`，含 typed receiver）、`@all_static_methods`（关联函数）；
- 典型用法：`#blanket(@all_ref_methods){Box}` 只委托引用 receiver 的方法，
  绕开 by-value 委托对包装类型的语义模糊（by-value 方法回落到 trait 默认实现）；
- 与其余 `@all` 族一样被 `#fill` / `#delegate` / `#blanket` 与 `-` 排除共享；
  `batch_trait!` 报错（需要 trait_def）。

### 注释、错误消息与文档全英文化

- **注释与错误消息全部改为英文**（源码、测试、ui fixture）——受众更广；消息中的
  DSL 记号（`` `@uint` ``、`` `#fill` ``、`` `@0` ``）保持不变；
- **文档语言策略确立**：开发期以中文 doc（`docs/zh-CN/`）为主文档记录改动，
  发布前翻译为英文放入英文 doc（`README.md`、`CHANGELOG.md`、`docs/tutorial.md`、
  `docs/architecture.md`、`docs/dev-changelog.md`）；0.6.2 已完成英文版初译，
  中文版继续作为开发态主文档演进；
- 文档中的代码示例不变（兼作 doctest——46 个全过）；
- 修复了 tutorial 中段级 `@trait` 示例的损坏围栏（`` `ust `` → `` ```rust ``），
  顺带纳入 doctest 覆盖。
## 0.6.1 (2026-08-05)

### 新特性：`@all_required*` / `@all_default*` 范围标记

- 指令范围按 trait item 的**默认实现状态**过滤（fn 带默认体 / const 带默认值 /
  type 带默认类型 = default；无默认 = required，impl 必须提供）：
  - `@all_required_methods` / `@all_required_constants` / `@all_required_types` / `@all_required`；
  - `@all_default_methods` / `@all_default_constants` / `@all_default_types` / `@all_default`；
- `#fill` / `#delegate` / `#blanket` 三指令与 `-` 排除通用；
- 典型用法：`#fill(@all_required_methods){...}` = 只实现必须的、默认方法保留
  trait 默认实现（此前需 `@all` + 逐个 `-name` 排除）；`@all_required*` 与
  `@all_default*` 组合可分别填充两类（required ∪ default = all）。

### 修复：`@` 常量先于 `<>` 配对（`@ <> # where` 预处理顺序）

- 此前管线为 `<> @ # where`：`batch_trait!` 里 `Vec<@inner>` 这类
  常量值含 `<...>` 的写法，`@inner` 被配对进尖括号组后不再展开，
  残留到输出报 `found '@'`（`@map = HashMap<u32, String>` 直接值
  恰好被定义处配对兜底，嵌套/引用场景暴露）；
- 修正为宏元层最外：`@` 展开先于 `<>` 配对，展开产物（含扁平
  `<...>`）统一由 angle_collect 配对；
- `batch_impl`/`batch_impl_only` 支持内置 `@` + `<>` + `#` + where；
  `batch_trait!` 支持自定义 `@` + `<>` + where（`#` 需 trait 定义，函数式
  宏不可用）。

### 宏元层完整化：`@` 是唯一宏元记号

- **`#all` 系删除，全部迁移为 `@all` 系**（`@all` / `@all_methods` /
  `@all_constants` / `@all_types` / `@all_required*` / `@all_default*`）：
  `#` 只剩指令名格式，范围选择归宏元层——`#fill(#all)` 写作
  `#fill(@all)`，减法不变（`#fill(@all, -foo)`）；
- `@all` 展开为 `[item, ...]` 列表；指令参数支持手写 `[a, b]` 与
  `-[a, b]` 排除；
- **trait 感知常量**（`#[batch_impl]` / `#[batch_impl_only]` 专属；
  `batch_trait!` 无 trait 定义、遇之报错）：`@trait`（本地 trait 名）、
  `@Cow`（`Cow<'_>` + 固有约束打包）；
- **blanket 包装约束谓词**：`{Cow<'_> where{@0: ToOwned + ?Sized, @0::Owned: @trait}}`
  ——解决 deref target ≠ T 的包装（`Cow` 的 deref target 是 `T::Owned`），
  `@0` 指目标泛型；普通 where 谓词中 `@N` 为通用位置引用（元组\n  `()^2 where{@0: Clone}` 等）；「`<>` 只留名字、约束全进 where」后约束合并 =
  并列谓词（零分析）；普通 impl 的 `<T: Clone>` 写法保持兼容。

### 文档修正：`batch_trait!` 的指令边界明确化


- 此前 `lib.rs` 文档与 `docs/tutorial.md` 声称 `batch_trait!` 的 spec 语法
  "与 `#[batch_impl]` 相同"——实际 `batch_trait!` **不支持 `#` 指令**
  （`#fill`/`#delegate`/`#blanket`/开放扩展），遇 `#` 直接报错；
- 原因：指令需要 trait 定义作签名真相源，`batch_trait!` 是函数式宏、拿不到
  定义。需要指令请用 `#[batch_impl]` / `#[batch_impl_only]`（与 `A<>` 照抄、
  泛型 bound 继承的既有限制同源）。
- 无行为变化：`batch_trait!` 支持 `@` 常量与全部类型 DSL，仅文档如实声明
  指令边界。

## 0.6.0 (2026-08-04)

### 新特性：`@` 常量系统（类型矩阵命名复用）

`@` 常量在预处理阶段展开为字面列表，与手写逐 token 等价：

- **内置名字族**：`@uint` / `@int` / `@float` / `@num` / `@scalar`
  （如 `#[batch_impl(@scalar)]` 一行生成 16 个 impl：u8..char）；
- **内置范围族**：`@u8..u128` / `@i8..i128` / `@f32..f64`（**含端点**，
  宽度校验；`@u8..u128` = `[u8, u16, u32, u64, u128]`）；
- **用户自定义**（仅 `batch_trait!`）：前导 `@name=值;` 段，后续段落跨
  trait 复用。值是**任意 token**（**懒展开**——原样入库，引用处拼接后递归
  展开），可直接写 DSL 运算（`@wrapped=[Box,Rc]^@num`）或链式引用其他常量
  （`@chain=@wrapped`）；循环引用（`@a=@a`）与前向引用（`@a=@b` 定义在后）
  在定义处报错；
- 未知 `@xxx`、范围端点非法、自定义与内置重名均 `compile_error!`。

### 新特性：`#blanket(methods){包装列表}` — 覆盖式委托

`#blanket(#all){&,Box,Rc}` 为每个包装类型生成一段完整委托 spec——免写包装
矩阵与委托体。先给内部类型实现 trait，再 blanket 覆盖包装
（`impl<T: Trait> Trait for Box<T>` 等）。

- **包装元素为任意类型表达式**：`&`/`&mut`/`Box`/`Rc`/`Arc`/自定义智能指针/
  嵌套（`Box^Arc:2` → `Box<Arc<T>>`）/预填（`Cow<'_>` → `Cow<'_, T>`）；
- **`:N` 深度标注**：委托体 `*` 数量 = N + 1（`Box^Arc:2` → `***self`），
  默认 1——宏不猜包装内部 Deref 层数，嵌套须显式标注；
- **泛型 trait 支持**（`trait Foo<X: Clone>`）：trait 形参照抄为 impl 泛型 +
  实参填参数名 + trait 级 where 谓词透传（`impl<X: Clone, T: Foo<X>>
  Foo<X> for 包装<T> where ...`）；
- **assoc type / const 委托**：`#all` 含 const/type 项时生成投影
  `type Item = <T as Foo<X>>::Item;` / `const N: Ty = <T as Foo<X>>::N;`——
  带必需关联类型的 trait 也能 blanket 覆盖；
- `*const`/`*mut`、`self`、空元素/非法 `:N` 报错，引导手写 `#delegate`；
  by-value receiver 方法委托语义依赖包装的 Deref/move 能力，维持全放行 +
  rustc 兜底（文档警示）。

### 行为变化

- 指令展开协议改为 `Vec<TokenTree>`（内部）：既有指令产物不变（单 `{...}`
  组），`#blanket` 等多产物指令成为可能。用户无感知。

### 文档

- README 精简为推销版（为什么要用它、心智模型、快速开始、特性一览），
  完整教程独立到 `docs/tutorial.md`，开发者文档独立到 `docs/architecture.md`
- CHANGELOG 拆分为本文件（作者视角）与 `docs/dev-changelog.md`（开发者视角）
- 教程新增 `@` 常量与 `#blanket` 章节；架构文档新增「语法域隔离」与
  「附着语义」章节

## 0.5.7 (2026-08-03)

### 新特性：trait 级 where 子句继承（自动生效，无需改代码）

`trait Foo<T> where T: Clone` 的谓词**全形态**继承到生成的 impl：

- **单一形参谓词**（`T: Clone`）合并进泛型 bound——`<T> Foo<T>` →
  `impl<T: Clone>`，与内联 bound（`trait Foo<T: Clone>`）同一条继承链路
  （同名继承 / 改名报错 / 引用检查全部复用）；
- **其余谓词原样透传**到 impl 的 where 子句：`T::Item: Clone`、`Vec<T>: ...`、
  生命周期谓词（`'a: 'b`）等全部覆盖，`<T>` 与 `A<>` 两种写法同效。

### 行为变化

- 此前复合谓词（`T::Item: Clone` 等）被静默丢弃，生成缺约束的 impl 导致
  rustc E0277（且定位模糊）；现在自动附加到 impl where，与手写等价。
  此前因此报错的代码升级后直接可用。
- 新增错误消息：`继承的 where 谓词 ... 引用形参 ...，请声明或手写 where`
  （改名场景引导）。
- 无破坏性变化；`batch_trait!` 无 trait 定义，不受影响。

## 0.5.6 (2026-08-03)

### 行为变化：孤立 `<` / `>` 报错

- 未配对的 `<`（缺少匹配 `>`）与多余的 `>`（缺少匹配 `<`）此前透传为垃圾
  token，现在报 `compile_error!`（非法输入）。

## 0.5.5 (2026-08-03)

### 新特性：`A<>` trait 泛型照抄

- `A<>`：空实参列表表示"实参与 bound 全部来自 trait 定义"——
  `trait Foo<T: Clone>` + `#[batch_impl(Foo<> ())]` 展开为
  `impl<T: Clone> Foo<T> for ()`，一行都不用写泛型；
- `A<绑定们>` 同款照抄：`Foo<Item=T>` 照抄位置实参 + 绑定原样保留；
- 仅 `#[batch_impl]` / `#[batch_impl_only]` 可用（需要 trait 定义）；
  `batch_trait!` 无 trait 定义，`A<>` 原样透传。

### 行为变化：改名 = 明确报错，绝不静默

- 实参 `X` 对应形参 `T`（有 bound）但名字不同、或继承的 bound 引用
  `'a`/`U` 等形参名而 impl 未声明同名——均报 `compile_error!` 引导
  （请改名或手写 bound）。此前改名场景静默退化为不继承，生成缺 bound 的
  impl 报 E0277。

## 0.5.4 (2026-08-03)

### 新特性：trait 泛型 bound 自动继承

`trait Foo<T: Clone>` 时，spec 中**未写 bound** 的 impl 泛型参数按名继承 trait
的同名参数内联 bound——`#[batch_impl(<T> Foo<T> Vec<T>)]` 直接生成
`impl<T: Clone> Foo<T> for Vec<T>`，无需手写（此前生成的 impl 缺 bound 报 E0277）。

- 写了 bound = 用户负责，宏不干预（sub trait 蕴含交由 rustc 验证）；
- 继承 `T: Clone` / `T: 'a` 等内联 bound；trait 级 where 子句不继承（0.5.7 起支持）；
- 仅 `#[batch_impl]` / `#[batch_impl_only]` 支持；`batch_trait!` 不继承。

### 新特性：指令参数列表减法 `-name`（取代 `#except`）

`#fill`/`#delegate` 参数新增 `-` 前缀排除项：保留列表减去排除列表，排除优先。
`#except(保留){排除}` 的双括号形式被取代并移除：

- `#fill(#all,-foo){body}` = 所有 item 除 `foo`
- `#fill(#all,-#all_methods)` = 仅 const + type 项
- `-` 后缺目标、排除后为空报 `compile_error!`

## 0.5.3 (2026-08-02)

### 新特性

- **`unsafe fn(...)` 类型**：`unsafe` 紧跟 `fn` 时修饰 fn 类型本身
  （`unsafe fn(u32)->u32`、`unsafe fn^(A,B)-C`）；`unsafe X`（X 非 fn，并列）
  报错（忘写 `^` 的笔误）；裸 `unsafe` 后跟 `^`/`-` 仍是 unsafe impl 标记。
- **开放扩展机制修复**：不认识的 `#name(args){body}` 展开为函数式宏调用
  `name!{(args){body} trait ...}`——把方法名列表、body 与整个 trait 交给
  用户的同名宏（"用户自定义的 `#fill`"，此前属性委托写法必然编译失败）。
- **指令减法 `#except(保留){排除}`**（0.5.4 被 `-name` 取代并移除）。

### 修复

- **`#delegate` 参数转发加固**：解构模式参数（`(a, b)` / `_`）无法委托转发，
  此前被静默丢弃生成错误调用，现报 `compile_error!`（含 trait 名与方法名）。
- **空范围诊断**：`()^3..2` 等空范围此前静默生成零个 impl，现报错。
- **尾随运算符静默吞段修复**：`A^`、`f32 Vec^-` 等尾随运算符此前整段静默
  消失（下游 E0599 定位模糊），现报 `compile_error!`。
- **空操作数严格化**：`-A`（左空静默吞段）、`^A`（生成垃圾类型）、`,A`、
  `A,,B` 均报错；尾随逗号（`A,`）与 `()`/`[]` 真实 token 不受影响。
- **指令参数逗号严格化**：`#fill(a,,b)` 等前导/尾随/连续逗号报错（此前静默跳过）。

### 行为约束：组合展开数量上限

`^N` / 笛卡尔积 / 范围批量等展开超过 1024 产物（如 `()^100000`、
`[A,B]^[C,D]^[E,F]`）报 `compile_error!`，防止误写挂死编译。

## 0.5.2 (2026-08-01)

### 新特性：数组/切片 builder

- `[]^T` → `[T]`（空基座包出切片）
- `[T]^N` → `[T; N]`（定长数组；`N` 可为数字字面量、const 泛型标识符、范围或列表）
- `<const N: usize> []-X-N` → `[X; N]`：`[]` 作 `-` 累加链基座，把整个类型矩阵
  包进 const 泛型定长数组
- `()^N` 的 fresh 泛型元组作为泛型实参/数组元素时自动外提
  （修复 `Box^()^N` 与矩阵嵌入的既有 bug）

## 0.5.1 (2026-07-31)

### 新特性：`where{...}` 后缀

- `where{...}` 跟在目标类型之后，为生成的 impl 添加 where 子句；多个会合并。
- 裸写 `where 谓词 {代码块}` 新语法（三个接口通用）：谓词区逗号不被 spec
  切分，`ident!{...}` 宏体不计入边界，多个 `where` 段可依次书写。

## 0.5.0 (2026-07-28)

### 新特性：`#[batch_impl_only]` 外部 trait 路径前缀

`#[batch_impl_only(#ext::mod::TraitName: usize, isize)]` 为外部模块中定义的
trait 生成 impl（路径末尾标识符必须与本地 dummy trait 名一致；
`#[batch_impl]` 不支持此前缀）。

## 0.4.2 (2026-07-27)

### 新特性

- **`#name{body}` 支持 const / type 项**：`#CONST{value}` → `const ... = value;`、
  `#Type{def}` → `type ... = def;`，不再局限于 fn。
- **`#fill` 扩展与 `#all` 标记**：`#fill` 可用于 fn + const + type；
  `#all` 变为所有 item；新增 `#all_methods` / `#all_constants` / `#all_types`。
- `#delegate` 仍仅支持 Fn，传入非 Fn 项报 `compile_error!`。

## 0.4.1 (2026-07-25)

- 修复自定义（开放扩展）宏未携带 trait_def 的问题。

## 0.4.0 (2026-07-25)

### 新特性：指令系统

| 指令   | 语法                      | 效果                                      |
|--------|---------------------------|-------------------------------------------|
| 单方法 | `#method{body}`           | `{fn method(签名) { body }}`              |
| 填充   | `#fill(args){body}`       | `{fn m1(sig){body} fn m2(sig){body} ...}` |
| 委托   | `#delegate(args){target}` | `{fn m1(sig){(target).m1(args)} ...}`     |

- `#fill(#all){body}` 表示 trait 的所有方法
- 指令与 DSL 运算符、`{body}` 连续附着、泛型、unsafe 等特性自由组合
- 仅 `#[batch_impl]` / `#[batch_impl_only]` 支持

### 新特性：`#[batch_impl_only]` 与 `{body}` 连续附着

- `#[batch_impl_only]`：丢弃 trait 定义、只输出 impl 块（trait 已在别处定义时用）
- `T{body1}{body2}` 正确递归附着

## 0.3.0 (2026-07-24)

### 完全重写

v0.3.0 是从零开始的完全重写。公开 API 和 DSL 语法与 v0.2.x 保持一致。
功能清单：

- `#[batch_impl]` 属性宏 + `batch_trait!` 函数式宏
- `^`（右结合）/ `-`（左结合）运算符：泛型应用、类型组合
- `[A, B, C]` 并列列表 + `{ body }` 独立/共享实现体合并
- `<T: Clone, Item=V>` 泛型参数与关联类型绑定
- `()^N` 元组生成 + `(<Bound>)^N` 带约束元组 + `(T1,T2)^N` 笛卡尔积 + 范围语法
- `&` / `&mut` / `*const` / `*mut` / `fn` / `self` / `unsafe` / `#[attr]` 前缀修饰符
- `fn(A,B)->C` 函数类型
- `HashMap<K>^V` 预填泛型追加
- `unsafe^T` 单条 unsafe + `unsafe trait` 自动 unsafe
- `compile_error!` 错误输出（不 panic、不 ICE）

### 修复（相对于 v0.2.x）

- `batch_trait!` 中 `fn(i32) -> bool` 等含 `->` 的 spec 不再误断段落边界
- `()^0` 正确生成空元组 `()`

## 0.2.2 (2026-07-20)

### 修复

- `fn^i32` 正确生成 `fn(i32)` 而非 `fn i32`
- 所有工具函数统一排除 `->` 中的 `>`（`HashMap^<u32>-String` 等含 `->` 的
  类型不再误判尖括号）

## 0.2.1 (2026-07-20)

### 修复

- **优先级**：`HashMap^K-V` 现在正确解析为 `HashMap<K, V>`（此前被解析为
  `HashMap<K<V>>`）。注意：`Box^Vec-u32` 仍是错误写法，应写 `Box^Vec^u32`
- `HashMap^<u32>-String` 中 `-String` 不再被静默丢弃
- `unsafe^#[attr]^T` 不再报"属性 ^ 的内部错误"
- `fn^(u32,i32)-usize` 正确生成 `fn(u32,i32)->usize`（此前返回类型被当参数追加）
- 嵌套 `fn^(u32,i32)^i64-usize` 不再丢失 `Fn` 前缀

## 0.2.0 (2026-07-19)

### 新功能

- **关联类型简洁写法**：`TraitName<AssocType=value>`（支持多绑定与复杂类型，
  可与 `^`/`-`/unsafe 组合）
- **独立/共享 body 合并**：`[A{bodyA}, B{bodyB}]{shared}`（支持多层嵌套）
- **元组生成规则修改**：`()^N` 生成带 N 个泛型参数的元组；`(T)^N` 生成长度
  N 的重复元组；`(T1,T2)^N` 笛卡尔积；范围语法 `()^M..N` / `()^M..=N`
- **`*const`/`*mut` 指针**：`*const^T` → `*const T`，支持链式
- **引用修饰符特殊行为**：`&^A^B` → `&A<B>`（先绑定再应用）
- **fn 关键字**：`fn^(A,B)` 创建、`fn(A,B)^T` 追加返回类型、`fn-(A,B)^N` 组合
- **`#[...]` 属性**：`#[attr]^T` 在 impl 块前添加属性

## 0.1.1 (2026-07-19)

### 新功能：预填泛型追加

- `A<B>^C` → `A<B, C>`（容器带预填泛型时 `^` 追加参数而非生成 `A<B><C>`）
- `[Box, Cow<'_>]^T` → `Box<T>, Cow<'_, T>`（列表支持）
- `-` 运算符自动受益：`HashMap-u32-String` → `HashMap<u32, String>`

## 0.1.0 (2026-07-19)

### 初始发布

- `#[batch_impl(...)]` 属性宏 + `batch_trait!(...)` 函数式宏
- `^`（右结合）/ `-`（左结合）运算符：泛型应用
- 元组生成：`()^N`、`(<Bound>)^N`、`(T1,T2)^N` 笛卡尔积、`()^M..N` 范围
- 泛型支持：impl 泛型（含 const）、trait 泛型、生命周期、泛型继承
- `unsafe^T` / `unsafe trait` / `batch_trait!(unsafe ...)` 
- 中文错误提示，`compile_error!` 而非 panic
