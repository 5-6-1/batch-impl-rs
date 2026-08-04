# 开发者变更记录

> 内部实现细节、重构、测试、CI；用户可见功能见 `CHANGELOG.md`。

## 0.6.0 (2026-08-04)

### 新特性：`@` 常量系统（src/consts.rs）

- 内置名字族（`@uint`/`@int`/`@float`/`@num`/`@scalar`）+ 范围族
  （`@u8..u128` 等，含端点、宽度/族/顺序校验），展开为 Bracket 列表与
  手写等价，走原管线（宏元层只做词法替换，不参与域内解析）
- `batch_trait!` 前导 `@name=值;` 定义段（`collect_user_consts`）：**懒展开**
  ——值任意 token 原样入库，引用处拼接后递归展开（`expand_consts` 引用分支
  先递归再 extend）；`check_value_refs` 定义处校验引用可见性（循环/前向
  引用拦截——懒展开下 `@a=@a` 会无限递归）
- 引用替换（`expand_consts`）递归进入 `Paren`/`Bracket`、透传 Brace 与
  `ident![...]`/`#[...]`（复用 `bracket_is_passthrough`）
- 管线位置：`angle_collect` 之后、指令预处理之前（两个入口各插一次；
  `batch_trait!` 在 `where_process` 之前）
- 教训×2：`expand_consts` 初版误加 `delimiter![none]` 分支把尖括号组
  （同值）当真实 None 组扁平化，已删；懒展开后值形态校验取消（B1/B2 的
  定义处拒绝语义被引用处 DSL 报错取代，评审认可）

### 新特性：`#blanket` 覆盖式委托

- `expand_directive` 返回类型 `TokenTree` → `Vec<TokenTree>`（指令可产出
  多 token；既有五种指令在分发处包 `vec!`，内部零改动）
- `expand_blanket`：**包装元素普适化**（任意类型表达式 + 可选尾 `:N` 深度
  标注，`parse_blanket_wrappers` 返回 `BlanketWrapper { ty, depth }`；
  `is_single_colon` 区分 `::` 路径）、fresh 泛型、逐包装生成
  `<T: Trait> 包装^T { 委托体 }` 多段 spec
- 委托体 `*` 数量 = depth + 1（`"*".repeat(depth + 1) + "self"` parse）；
  目标类型 = 包装 `^T`（`Box^Arc:2` → `Box<Arc<T>>`、`Cow<'_>` → `Cow<'_, T>`）
- **泛型 trait**：trait 形参照抄为 impl 泛型（形参在前、fresh `T` 在后，
  `T: Trait<X>` 反序 E0401）+ trait 实参填参数名 + where 谓词透传；
  spec 的 trait 名部分仅泛型时输出（非泛型省略——`Trait &^T` 前缀目标
  跟在 trait 名后无法解析，回归曾破坏 `{&,Box,Rc}`）
- **assoc type/const 委托**：`TraitItem::Fn` 窄匹配放开，Type/Const 走
  `build_from_item` 既有输出形态，body 用 `<T as Trait<X>>::name` 投影
- 关键修复×2：blanket 在 `angle_collect` 之后运行——泛型声明手动构造尖括号组
  （`Group::new(delimiter![<>], ...)`）；body 是 Brace 组（angle_collect 不进入），
  其内 `Cow<'_>` 等扁平 `<...>` 补一次 `angle_collect` 配对
- 坑：`quote!(#tp.ident)` 字段访问插值（`.ident` 当字面量），先取引用再插值
- 边界：`*const`/`*mut` / `self` / 空元素 / 非法 `:N` 报错引导手写
  `#delegate`；默认 depth 1（宏不猜 Deref 层数）；by-value receiver 放行
  （Deref/move 语义信息不对称，rustc 兜底）

### 测试与文档

- dsl 第 35/36 节（const 系统、blanket 双属性叠加）；ui 新增 fixture
  （const_unknown / const_range_bad / blanket_ptr / blanket_bad_depth；
  blanket_generic 随泛型 trait 支持移除；const_cycle / const_forward 见评审修复节）
- architecture.md：模块图加 consts.rs、管线更新（const 展开、多 token 指令）、
  域隔离表格宏元层落地、新增「附着语义」章节
- tutorial.md：第 7 章 `#blanket` 小节、第 11 章 `@` 常量小节

### 评审修复（发布前）

- **F1**：`cargo +nightly fmt` 修复 consts.rs / preprocess/mod.rs 格式差异
- **F2**：dsl.rs `BlanketInc` dead_code（clippy -D warnings 阻断）——`b.inc()`
  走 Deref 到 u16 自身 impl、blanket `&mut` impl 从未被调用；测试改为 UFCS
  直接测 blanket 委托路径（`&mut u16` 同时命中两个 impl 需消歧）
- **F3**：`@name=值;` 定义段写在 trait 段之后时，`try_expand_at` 定义段分支
  按上下文区分诊断——batch_trait! 报「常量定义必须位于所有 trait 段之前」，
  batch_impl/batch_impl_only 保留「不支持自定义常量」
- **F4**：blanket 泛型 bound `T: Trait<X>` 的实参扁平 `<A, B>` 会被
  `split_at_depth0` 在逗号处错误切分（`T: Two<A` / `B>`），初版靠渲染幂等
  侥幸正确（脆点）；修复为**实参组化**（`t_bound` 与 `trait_part` 同款
  `Group::new(delimiter![<>], ...)`），解析即正确不依赖幂等；dsl 38 的
  `Two<A, B>` 用例回归锁定；parse/generic.rs 注释改为「组内宏生成尖括号
  必须预配对」的通用警告

- **B1**：`collect_user_consts` 的 `@` 引用值校验 `consumed == value.len()`
  ——`@a=@num garbage` 报"引用后有多余 token"，不再静默丢弃尾随内容
  （**已被懒展开取代**：值形态放开为任意 token，见本版本新特性节）
- **B2**：常量**列表**值内嵌 `@`（`[@uint, u16]`）在定义处拒绝——接受但不
  展开会推迟到使用处才报错（诊断远离源头）；列表是原子值，请用 `@name` 形态
  （**已被懒展开取代**：列表值内嵌引用现在正常展开，见 dsl 38）
- **B3**：`#blanket` 的委托 bound 改用 `trait_full_path`——`#[batch_impl_only
  (#ext::Trait: ...)]` 路径前缀场景裸 dummy 名解析不到（E0412/E0277）；
  `expand_tokens`/`expand_directive`/`expand_blanket` 签名链加 `trait_full_path`
  参数（fuzz 同步）
- **B4**：未知 `@` 常量诊断在 batch_trait! 场景追加"用户常量须在引用前定义"
  （懒展开后由 `check_value_refs` 的定义处可见性校验接管，见新特性节）
- **B6**：`contains_at` 递归进所有组（`[Foo<@uint>]` 的 `@uint` 被 angle_collect
  配对进 None 组，扁平检查会漏过）——**已被 `check_value_refs` 取代**（懒展开
  后定义处统一做引用可见性校验，递归进所有组）
- 测试：regression 加路径前缀 + blanket pass 用例（`cmp_path_prefix_blanket`，
  `&u8` 与 u8 自身 impl 的方法歧义用 UFCS 消歧）；ui 加
  const_cycle / const_forward 两个 fixture（循环/前向引用定义处报错）

### 文档体系重构（并入自原 0.5.8）

- README 重写为推销版（669 → 117 行）：为什么用它 / 心智模型 / 快速开始 /
  特性一览表 / 链接
- 教程独立 `docs/tutorial.md`（原语法参考 + 组合拳重排为 13 章渐进式，
  lib.rs 增加 `#![doc = include_str!(docs/tutorial.md)]`，docs.rs 首页 =
  推销 + 教程，教程代码块全部进 doctest）
- 开发者文档独立 `docs/architecture.md`（架构图、关键设计决策、错误机制、
  测试矩阵、发布流程）
- CHANGELOG 拆分为用户版（CHANGELOG.md）与开发者版（本文件），0.1.0 →
  最新全部历史条目分类迁移
- 注意：rustdoc 对无语言标注代码块默认按 rust 编译（`<impl-泛型>...` 骨架
  需 `text` 标注）

## 0.5.7 (2026-08-03)

### `delimiter!` 分隔符拼写宏

- 定义于 `preprocess/mod.rs` 顶部（经 `#[macro_use]` 导入 crate 根），用源码
  分隔符拼写统一取缔散落的 `Delimiter::*` 字面量，调用统一用 `[]` 定界
- `Delimiter::None` 两种语义用两种拼写区分：`delimiter![<>]`（尖括号组载体）
  与 `delimiter![none]`（真实透明组）；全库 43 处收敛
- 修 angle.rs 模块文档悬空的 `ANGLE_BRACKET` 引用
- proc-macro crate 禁止 `#[macro_export]`，宏无法定义在 `angle.rs` 并全
  crate 可见，故置于父模块顶部（文本作用域要求声明先于所有使用者）

### Bracket 守卫对齐

- `expand_tokens` 与 `where_process` 的 Bracket 递归守卫补 `#`（此前仅排除
  `ident![...]`，`#[...]` 属性内的 `#name{body}` 会被误当指令展开报错；
  与 `angle_collect` 的属性守卫对齐）

### lib.rs 拆分（632 → 202 行）

- `expand.rs`：入口实现 + 公共管线 `run_pipeline`（解析 → 生成 → 尖括号组
  还原；`angle_collect` 与裸 where 改写不进入管线——配对破坏性、where 须
  先于 `A<>` 展开）
- `trait_bounds.rs`：TraitBounds + syn AST 引用收集
- `empty_generics.rs`：`A<>` 照抄展开
- `angle_tests` 迁入 `angle.rs`；`crate::TraitBounds` 路径经 `pub(crate) use`
  保持兼容
- 错误机制分工说明：入口层 `Result` 传播 vs DSL 层 `Ty::Error` 透传；
  `batch_trait!` 段级错误统一 `return Err`

### syn AST 引用收集（where 谓词）

- 新增 `syn` 的 `visit` feature：单段路径与泛型实参是形参引用位置；
  `::` 后路径段（`A::B` 的 `B`）、关联类型绑定名（`dyn Trait<Item = T>`
  的 `Item`）、HRTB binder（`for<'a>` 的 `'a`）天然排除——替换 `bound_refs`
  的 token 扫描（顺带修掉内联 bound 的 HRTB 误报）
- 补 `visit_expr` 收集 const 泛型实参 / 数组长度（`[T; N]` 的 `N`，实测发现
  漏报会静默生成引用未声明名字的代码）；impl 泛型名 `const N` 归一如 `N`
- `TraitBounds.extra_predicates`：未合并谓词（token + 引用的形参名），
  codegen 引用检查后附加到 impl where

### 其他

- CI：MSRV job 补 doctest（`--doc` 不能与其他选择项混用，拆两步）
- 测试：angle 单测（属性/宏体守卫、渲染嵌套组重建、span 保留不可测说明——
  fallback 模式 `Span::mixed_site()` 即 call_site）；regression 补
  `batch_trait!` 的 `A<>` 透传；dsl 第 34 节覆盖矩阵；ui 新增
  `rename_where.rs` / `where_const_ref.rs`；codegen 单测锁定 `WhereArr<>`
  展开（防"测试过但 IDE 展开含 compile_error"的缓存类误报）

## 0.5.6 (2026-08-03)

### src 按层分目录

- 管线分层：`parse/`（解析器 + 原子层 + 泛型）、`preprocess/`（指令 + 辅助 +
  裸 where + 尖括号组）、`ast/`（Ty 定义 + 渲染）、`apply/`（Apply trait +
  元组容器）、`codegen/`；同名文件并入 `mod.rs`（消除
  `module_has_same_name`），子模块经 `pub(crate) use` 重导出，外部路径不变

### 尖括号组预处理（angle.rs）

- proc-macro2 只对 `()`/`[]`/`{}` 分组，`<>` 是扁平 Punct——新增
  `angle_collect` 在管线入口一趟扫描：真实 `None` 组扁平化 + 扁平 `<...>`
  配对为 `None` 组（`->` 箭头不参与）；`Paren`/`Bracket` 递归进入、
  `Brace` 不进入（body 透传）、`ident![...]` 宏体 / `#[...]` 属性不进入
- `render_angles` 输出侧镜像（`None` 组 → `<...>` 扁平），重建 `Paren`/
  `Bracket` 时保留原 span（修复 doc 属性 span 变 call_site 的 clippy 诊断
  映射问题）
- 收尾：孤立 `<`/`>` 报错（解锁下游深度逻辑删除）；`scan_with` /
  `scan_body_boundary` / 路径扫描删除 `<>` 深度分支
- fuzz 全管线补 `angle_collect`

## 0.5.5 (2026-08-03)

### `A<>` 照抄实现

- `TraitBounds` 重写为位置结构（`TraitParam`: name / bound / refs）
- `bound_refs` 保守 token 级引用检测（宁可误报拒绝自动继承，绝不生成错代码）
- `expand_empty_trait_generics` 预处理扫描（深度 0 的 `Ident<>`，`->` 箭头守卫）
- 取代初版"生命周期按名匹配 + 退化为不继承"：改名场景从静默退化升级为明确报错

## 0.5.4 (2026-08-03)

### `-name` 减法实现

- `parse_name_tokens` 重写为 keep/exclude 双列表 + `#` 标记展开
  （`parse_marker` / `parse_minus_target` 辅助），`#except` 分支移除

### bound 继承实现

- `extract_trait_bounds` 从 trait generics 提取 name→bound 映射（Punctuated
  经 ToTokens 渲染 `A + B`），经 `parse_batch_trait_entry` 传入
  `generate_impl` 对 `(name, None)` 参数补 bound
- 修复 `quote!(#tp.bounds)` 陷阱：quote 插值不支持字段访问（会把 `.bounds`
  当字面量），改用先取引用

### 其他

- 发布物冒烟验证（首次验证真实发布物可用）
- README 快速开始版本号修复（0.5.1 → 0.5.4，crates.io 版本不可变故重新发布）

## 0.5.3 (2026-08-02)

### 重构与内部实现

- **preprocess 返回类型收敛**：指令展开产物收敛为恰好一个 `{...}` 组 token
- **指令参数解析重构**：`parse_names_from_tokens` 的别扭写法（逗号编码成
  `Err(None)`）改为普通迭代收集
- **fuzz 扩到全管线**：`full_pipeline_no_panic` 随机 token 流跑完整管线
- **`Apply` trait 重构**：右操作数"结构上下文"提前分发下沉为默认方法
  （Array 分发 / Group 透明 / WithCode、WithWhere 透传 / WithType 外提 /
  Range 展开 / Error 透传）；移除 `TyArray` 不可达笛卡尔积分支与 `TyFn`
  不可达 Group 分支；`trait Apply: Clone + Into<Ty>`（分发需复用左操作数）
- **`Ty::expand` 返回值改为显式枚举**：`enum Expand { Leaf, Many }`
  （原 `Result<Vec<Ty>, Ty>` 以 `Err` 表示叶子的反直觉设计）
- **组合展开数量上限**：`MAX_EXPAND = 1024`，`tuple_pow` / `pow_cartesian`
  （每轮产物数）/ `map_range` / `TyArray` 笛卡尔积分支校验，
  `apply::check_expand_limit` 统一入口
- **数组链式展开产物上限**：`count_leaves` 叶子数校验
- **元组笛卡尔积 bound 修复**：`instantiate_combo` 误把参数名当 bound
  （`(A: Clone, T)^N` 生成 `_Param: A`），改为保留真正的 bound
- **逻辑精简重构**（行为零变化）：`Ty::expand` 包装样板抽为
  `expand_wrapped` / `expand_rebuild`；指令展开骨架合并为 `expand_many`
- **文档漂移修复**：README 元组生成 u8 范围删除、测试矩阵计数更新、
  补充 unsafe fn / `#except` / 操作数严格性说明

### 修复（内部）

- `#delegate` 参数转发加固：`collect_call_args` 对非标识符模式返回错误
- 空范围诊断：`map_range` 对空范围报错
- 尾随运算符静默吞段修复：Dash/Caret 分支空操作数报错
- 空操作数严格化：左空检查 + 前导/连续逗号在 3 个入口判定
- 指令参数逗号严格化

## 0.5.2 (2026-08-01)

### 测试与工程

- **解析器 fuzz 验证**：`src/fuzz.rs`（proptest）随机 token 喂
  `where_process` / `parse_item`，断言不 panic
- **发布卫生**：`#![forbid(unsafe_code)]`、`#![deny(missing_docs)]`、
  修复 Windows MSVC `linker_messages` 告警
- **CI**：GitHub Actions（fmt / clippy -D warnings / test / doc，
  stable + MSRV 1.93 双工具链）

### 数组/切片 builder（`TyPrimitiveArray`）

- 合并 `TySlice` 与 `TyFixedArray` 为
  `TyPrimitiveArray(Option<Box<Ty>>, Option<TokenStream>)`
- `()^N` fresh 泛型元组自动外提（`T^<A>X` => `<A>(T^X)`，嵌套 `WithType`
  参数并入 impl 泛型）
- `TyNum` / `TyRange` 由 `u8` 改为 `usize`

## 0.5.1 (2026-07-31)

### where 支持实现

- `where{...}` 后缀：`TyWithWhere` / `TyWhere` 节点，codegen 合并到
  impl 的 where 子句
- 裸 where 改写：新增 `where_process.rs`（指令预处理之后、DSL 解析之前），
  边界判定排除 `ident!{...}` 宏调用体与尖括号内代码块

## 0.5.0 (2026-07-28)

### 工程

- `try_parse_path_prefix` 状态机（要求至少一个 `::`，避免 `#Display: ...`
  歧义）
- `Spacing::Joint` 精确检查（`::`、`->`、`..` 防相邻不粘连标点误判）
- Range 处理集中化（`Apply for Ty` 外层 match 统一右侧 Range 展开）
- 模块级文档（`//!`）全量补齐
- 模块拆分：`scan.rs` / `parse_atom.rs` / `generic.rs` / `types_render.rs` /
  `apply_tuple.rs` / `batch_trait_entry.rs` / `path_prefix.rs` /
  `preprocess_helpers.rs`

## 0.4.2 (2026-07-27)

### 工程化重构

- `apply::trait Type` 重命名为 `trait Apply`
- `rustfmt.toml`（edition=2024、max_width=75 等），PR 要求
  `cargo +nightly fmt --check`
- `src/diagnostic.rs`：唯一 `compile_error_str(msg)` 构造器（删除两份同名
  实现，防诊断构造点漂移）
- `ScanMode { Lossy, Strict }` + 单一 `scan_with`（消除两份近似但行为不同的
  `<>` 深度循环）
- `extract_impl_parts` 的 `WithType` 分支 append → prepend（
  `<A>[<B>T1, <C>T2]` 现输出 `impl<A, B>` 与 `impl<A, C>`）
- 错误加固：`expand_tokens` 两处 `peek().unwrap()` 替换为 `let Some else`；
  `tuple_pow` 单元素分支 `.unwrap()` 改带消息 `expect`
- 入口收敛：`extract_trait_path` / `extract_last_ident` 内联进 `batch_trait!`
- 测试体系：`tests/dsl.rs`（20 个）+ `tests/ui.rs`（8 fail + 1 pass，trybuild）
- 测试与示例重组：删除 examples 4 个测试文件（~4800 行），新增
  `examples/quickstart.rs` + `tests/regression.rs`

### 其他

- `expand_delegate` 的 `todo!("error")` 替换为 `compile_error!`
- preprocess.rs 注释与 `get_trait_item` 错误信息更新

## 0.4.1 (2026-07-25)

- 修复自定义宏未携带 trait_def 问题

## 0.4.0 (2026-07-25)

### 指令系统实现

- 新增 `preprocess.rs`：指令预处理模块，仅递归展开 `[...]`（Bracket）Group
- `expand_tokens` / `expand_directive` 返回 `Result`，错误输出
  `compile_error!` 而非 panic
- 全库零 `panic!` / `unreachable!`：AST 层 `Ty::Error` 变体经 ToTokens 输出；
  预处理层 `parse_method_names_from_tokens` / `get_trait_method_sig` 返回
  `Result`
- 指令扩展性：不认识的 `#name` 委托给 Rust 属性宏系统（0.5.3 改为函数式
  宏调用）
- `examples/my_tests.rs`：36 项指令测试

## 0.3.0 (2026-07-24)

### 完全重写

- 从零开始重写，公开 API 与 DSL 语法与 v0.2.x 一致，内部与旧版无代码联系
- 架构：`lib.rs`（入口 + 共享驱动）/ `preprocess.rs` / `parse.rs` /
  `types.rs` / `apply.rs` / `codegen.rs`
- 解析模型：`Cursor<'a>` 借用切片游标 + 优先级攀爬（`Semi` < `Comma` <
  `Dash` < `Caret`），`scan_stop` 统一处理 `<>` 深度与 `->` 守卫；
  任意 Rust 类型透传为 Primitive 节点
- AST 设计：`Ty` 枚举 20 个变体（叶子 / 包装 / 容器三类）
- 运算符语义：`Type` trait 的 `apply(self, o)`（`^` 右结合、`-` 左结合、
  数组分发、元组生成）
- 测试：tests 95+ 项 / ds_tests 56+ 项全部通过，clippy 零警告

## 0.2.2 (2026-07-20)

### 修复与代码审查

- `fn^i32` 自动生成括号
- 统一 `->` 处理（`has_top_level_char` / `parse_balanced` /
  `find_top_level_colon` / `split_at_punct` 排除 `->` 中的 `>`）
- P0：`split_raw` 检测多余 `>`；`parse_balanced` 详细错误（"未闭合的 `<`（还有
  N 层）"）
- P1：`expand_nested_bracket` 注释（`unwrap_count - 1` 语义）；
  `generate_tuples` 返回 Result（笛卡尔积超限）；`batch_trait!` 空路径检查

## 0.2.1 (2026-07-20)

### 修复（BUG-1/2/3 与优先级）

- BUG-1：`expand_caret` 右侧在第一个顶层 `-` 处分割（`^` 优先级高于 `-`）
- BUG-2：`parse_target_items` 丢弃 `<>` 之后内容（`parse_balanced` 的 pos 被
  丢弃）
- BUG-3：`expand_single` 未过滤 Attribute/Unsafe 前缀（`unsafe^#[attr]^T`）
- fn 类型优先级：`fn^(u32,i32)-usize` 的 `-` 作为返回类型
- 嵌套 caret 保留 `Fn` 前缀

### Code Quality

- `ImplSpec::new()` 构造器；`expand_caret` 拆出
  `expand_bracket_with_comma` / `expand_nested_bracket`；`dash_append` 拆出
  fn 处理；`#![allow(linker_messages)]`

## 0.2.0 (2026-07-19)

### 实现细节

- `ImplSpec` 新增 `assoc_bindings` / `attributes` 字段
- `PrefixItem` 新增 `ConstPtr` / `MutPtr` / `Fn` / `Attribute` 变体
- `parse_segment` 解析 `TraitName<Item=T>` 时分离关联类型绑定
- 测试：macro-test 113 / ds-test 15 / 一致性 / 嵌套 / 并行

## 0.1.1 (2026-07-19)

### 实现细节

- `PrefixItem::Container` 增加 `prefill` 字段；`parse_single_prefix` 识别
  `Ident<...>`；`apply_caret` 预填泛型追加；`append_to_generic_container`
- README 优先级说明；移除 Planned 部分

## 0.1.0 (2026-07-19)

### 初始发布

- 安全性：递归深度限制（128 层）、`byte_range()` 稳定位置后缀、
  笛卡尔积组合数上限（1024）
- 错误处理：中文提示、保留原始 Span、`compile_error!` 而非 panic
- 测试：macro-test 99 / ds-test 15
