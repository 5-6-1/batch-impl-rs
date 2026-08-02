# Changelog

## 0.5.5 (2026-08-03)

### 修复：生命周期 bound 继承在改名场景引用未声明生命周期（E0261）

`trait Lifetime<'a, T: 'a>` + `#[batch_impl(<'b, T> Lifetime<'b, T> ())]` 此前把
trait 的 `T: 'a` 原样继承进 impl，生成 `impl<'b, T: 'a>` 引用未声明的 `'a`
（E0261，错误指向 trait 定义、无从修复）。现改为**生命周期 bound 按名匹配**：

- `T: 'a` 仅当 impl 声明了同名生命周期 `'a` 时才继承（同名场景行为不变）
- `T: 'static` 全局可用，照常继承
- 改名场景退化为不继承：rustc 报 `T: 'b` 不满足（E0309）并直接建议
  `<'b, T: 'b>`——可读、可修复
- 实现：`TraitBounds` 拆为 `types`（trait bound 按名继承）与 `lifetimes`
  （生命周期 bound 按名匹配 impl 声明的生命周期）；修复 `String` 插值陷阱
  （quote 插值 String 会渲染为字符串字面量 `"'static"`，改存 TokenStream）
- 测试：dsl 第 32 节新增改名手写 / `'static` / 混合 `Clone + 'a` 用例

## 0.5.4 (2026-08-03)

### 指令参数列表减法 `-name`（取代 `#except`）

`#fill`/`#delegate` 参数新增 `-` 前缀排除项：保留列表减去排除列表，排除优先。
`#except(保留){排除}` 的双括号形式被其取代并移除（0.5.3 当天发布、使用者为零，
破坏面最小）：

- `#fill(#all,-foo){body}` = 所有 item 除 `foo`（`#fill(#except(#all){foo})` 的等价物）
- `#fill(#all,-#all_methods)` = 仅 const + type 项（标记也可被排除）
- `-` 后缺目标（`a,-`）、排除后为空（`#all,-#all`）报 `compile_error!`
- `-` 只在指令参数域生效（参数此前只解析标识符/逗号，`-` 为自由符号），
  与类型 DSL 的 `-` 连接运算符互不干扰
- 实现：`parse_name_tokens` 重写为 keep/exclude 双列表 + `#` 标记展开
  （`parse_marker` / `parse_minus_target` 辅助），`#except` 分支移除
- 测试：`tests/dsl.rs` 第 30 节（标识符排除 / 标记排除 / 显式列表）、
  `tests/ui/minus_bad_target.rs`、`tests/ui/minus_empty.rs` 锁定诊断；
  `except_missing` / `except_empty` fixture 删除

### trait 泛型 bound 自动继承

`trait Foo<T: Clone>` 时，spec 中**未写 bound** 的 impl 泛型参数按名继承 trait
的同名参数内联 bound——`#[batch_impl(<T> Foo<T> Vec<T>)]` 直接生成
`impl<T: Clone> Foo<T> for Vec<T>`，无需手写（此前生成的 impl 缺 bound 报 E0277）。

- 规则：**写了 bound = 用户负责，宏不干预**——sub trait 蕴含（`trait B: A` 使
  `T: B` 隐含 `T: A`）宏无法推理，交由 rustc 验证；未写才继承，零噪音、零合并
- 继承 `T: Clone` / `T: 'a` 等内联 bound；trait 级 where 子句不继承（第一版范围）
- 仅 `#[batch_impl]` / `#[batch_impl_only]` 支持（trait 定义在手边）；
  `batch_trait!` 无 trait 定义，不继承
- 实现：`extract_trait_bounds` 从 trait generics 提取 name→bound 映射
  （Punctuated 经 ToTokens 渲染 `A + B`），经 `parse_batch_trait_entry` 传入
  `generate_impl`，对 `(name, None)` 参数补 bound
- 修复实现中 `quote!(#tp.bounds)` 的陷阱：quote 插值不支持字段访问
  （会把 `.bounds` 当字面量），改用 `to_token_stream`
- 测试：`tests/dsl.rs` 第 32 节（Clone 继承 / 用户例 `T: SupB` 不干预 /
  生命周期 `T: 'a` 继承）

### 发布物冒烟验证

临时 crate 依赖 crates.io 发布的 `batch-impl = "0.5.3"`，覆盖基础/泛型/元组/
指令/delegate/unsafe fn/where/batch_trait 运行通过——此前测试均走本地 path 依赖，
本次首次验证真实发布物可用。

### 修复 README 快速开始版本号

README 快速开始示例的依赖版本号停留在 `0.5.1`，与已发布版本不符（0.5.3 发布时
漏改）。crates.io 版本不可变，README 随包发布，故修正后以 0.5.4 重新发布。

- README 依赖示例更新为 `batch-impl = "0.5.4"`
- 代码零变化（纯文档 + 版本号）

## 0.5.3 (2026-08-02)

### 修复开放扩展机制（未知 `#name` 指令）

`preprocess.rs` 对不认识的 `#name(args){body}` 展开为一个 `{...}` 代码块，
内容是**函数式宏调用**——把方法名列表、body 与整个 trait 交给用户的同名宏：

```
#foo(args1){args2}  →  { foo!{(args1){args2} trait T { ... }} }
```

- 正确性：函数式宏调用 `foo!{...}` 在 impl body / 顶层都是合法项位置，rustc
  会在 impl 内展开它生成 fn 定义——不涉及"trait 进 impl"（此前属性 + trait
  内嵌 impl 的写法必然编译失败）
- 语义：这是"用户自定义的 `#fill`"——`#fill`/`#delegate` 是库读 trait 生成
  fn 定义，开放指令把同样的事交给用户宏；每个类型可各挂一个
  （`usize #foo(...){...}, isize #foo(...){...}`），trait 定义不重复
- 保留 `parse.rs` 裸代码块分支与 `codegen.rs` 原始 item 注入：仅服务"指令独立
  成整个 spec"的退化形态（顶层输出宏调用）
- 测试：`tests/dsl.rs` 第 28 节用函数式宏 `#[batch_preprocess_test]` 验证——`usize
  #batch_preprocess_test(add,inc){*self+1}` 生成 `impl AddInc for usize` 的两个方法

### preprocess 返回类型收敛

指令展开的每个产物恰好是一个 `{...}` 组 token，`expand_directive` /
`expand_single` / `expand_fill` / `expand_delegate` 的返回类型由
`Result<Vec<TokenTree>, TokenStream>` 收敛为 `Result<TokenTree, TokenStream>`，
`expand_tokens` 内指令分支由 `extend` 改为 `push`。消除无意义的 `Vec` 包装。

### `#delegate` 参数转发加固

`collect_call_args` 原先只收集纯标识符模式，解构模式（`(a, b)` / `_`）被静默
丢弃，生成错误的委托调用。现改为：遇到非标识符模式返回包含模式文本的错误，
`expand_delegate` 输出 `compile_error!`（含 trait 名与方法名）。
新增 `tests/ui/delegate_pattern_arg.rs` 锁定诊断。

### 指令参数解析重构

`parse_names_from_tokens` 原用 `Result<Ident, Option<TokenStream>>` +
`filter_map` 的别扭写法（把逗号编码成 `Err(None)`）。改为普通迭代收集；
新增"逗号过滤后参数仍为空则报错"（`#fill(,)` 不再静默生成空实现体）。

### 空范围诊断

`apply_tuple::map_range` 对空范围（起始不小于结束，如 `()^3..2`）原先静默
生成零个 impl。现输出 `compile_error!` 提示范围为空。
新增 `tests/ui/empty_range.rs` 锁定诊断；`()^0`、`()^0..3` 等合法用法不受影响。

### 组合展开数量上限（防编译挂起）

`^N` / 笛卡尔积 / 范围批量等展开操作可能指数级膨胀（如 `(T1,..,Tk)^N`、
`[A,B]^[C,D]^[E,F]`、`()^100000`），误写会挂死编译。新增
`types::MAX_EXPAND = 1024`（对齐 v0.1 上限），在 `tuple_pow`（`()^N`/`(T,)^N`）、
`pow_cartesian`（每轮产物数）、`map_range`（范围长度）、`TyArray` 笛卡尔积分支
校验，超限输出 `compile_error!` 中文诊断。新增 `apply::check_expand_limit` 统一入口。

### `Ty::expand` 返回值改为显式枚举

`expand` 原用 `Result<Vec<Ty>, Ty>` 且以 `Err` 表示"不可展开的叶子"（反直觉）。
改为 `enum Expand { Leaf(Ty), Many(Vec<Ty>) }`，语义自明；仅 `types.rs` 与
`batch_trait_entry` 的摊平循环两处改动。

### 指令参数逗号严格化

`parse_names_from_tokens`（`#fill`/`#delegate`/开放指令共用）原先静默跳过任意
逗号（允许前导/尾随/连续）。现改为：前导/尾随/连续逗号报
`compile_error!`（"逗号位置不合法"），避免 `#fill(a,,b)` 等笔误被静默吞掉。
新增 `tests/ui/fill_bad_comma.rs` 锁定诊断。

### fuzz 扩到全管线

`src/fuzz.rs` 新增 `full_pipeline_no_panic`：随机 token 流跑完整管线（指令预处理
→ where 改写 → DSL 解析/展开 → `generate_impl`，含 apply/expand/codegen），
断言任意输入不 panic，作为后续重构的安全网。

### `Apply` trait：右操作数提前分发下沉为默认方法

`trait Apply` 的 `apply` 改为**默认方法**，承担右操作数"结构上下文"的提前分发
（Array 分发 / Group 透明 / WithCode、WithWhere 应用透传 / WithType 泛型外提 /
Range 展开 / Error 透传）；各变体的组合语义移入 `apply_help`，`impl Apply for Ty`
只保留左分发（`match self` → 各变体 `apply_help`）。

- 提前分发从"仅 `impl Apply for Ty` 隐式承担、各变体实现隐式依赖它预先分发"
  升级为 **trait 契约**——任何 `Apply` 实现自动获得，`apply_help` 的右操作数
  恒为普通类型，不可能误处理数组/组
- 移除 `TyArray::apply` 中不可达的笛卡尔积分支（数组-数组由默认 Array 分支
  逐层分发 + `expand` 摊平，见 README 顺序更正）与 `TyFn::apply` 中不可达的
  Group 分支（`fn^` 右侧必须元组的规则不变）
- `trait Apply: Clone + Into<Ty>`：分发需复用左操作数 `self`；裸代码块/裸
  where 作为右操作数时需把左操作数装回目标类型
- 行为零变化（全量测试通过，fuzz 全管线兜底）

### 尾随运算符静默吞段修复

`parse.rs` 的 `Dash`/`Caret` 分支：`-`/`^` 后缺操作数（如 `f32 Vec^-`、`A^`）原先
经 `?` 传播返回 `None`，导致**整个段被静默丢弃**（后续 spec 正常、出错段无声消失，
用户只看到下游 E0599）。现改为报 `compile_error!`（"`-` 后缺少操作数（如 `T-U`）"）；
`^`/`-` 后紧跟深度 0 停止符的空操作数同样报错（`()`/`[]` 等 Group 是真实 token，
不受影响）。新增 `tests/ui/dangling_operator.rs` 锁定诊断。

### 数组链式展开产物上限

`[A,B]^[C,D]^[E,F]^...` 的产物随 `^` 链**指数增长**（中间数组每个都小、叶子数
翻倍），此前无上限。默认 `apply` 的 Array 分支新增 `types::count_leaves` 叶子数
校验，超 [`MAX_EXPAND`] 报 `compile_error!`。

### 元组笛卡尔积 bound 修复

`apply_tuple::instantiate_combo` 原先误把 TypeParam 的**参数名**当 bound
（`(A: Clone, T)^N` 会生成 `_Param: A` 而非 `_Param: Clone`）。改为保留真正的 bound。

### 指令减法 `#except(保留){排除}`

`#fill`/`#delegate` 的参数新增 `#except(保留){排除}` 列表减法：保留列表减去排除
列表，两列表各自是 `#all` 系列标记或逗号分隔的 item 名列表。

- `#fill(#except(#all){skip_me}){body}` = 所有 item 除 `skip_me`——被排除项走
  trait 默认实现，不被批量生成
- `#except(#all){#all_methods}` = 仅 const + type 项
- 两个括号参数缺一报 `compile_error!`；排除/保留列表为空报错
- 实现：`preprocess_helpers.rs` 抽出 `parse_name_tokens(tokens, trait_def, what)`
  共享标记+标识符列表解析，`what` 携带诊断上下文；主路径消息措辞不变
- 测试：`tests/dsl.rs` 第 30 节（排除项走默认值验证）、`tests/ui/except_missing.rs`、
  `tests/ui/except_empty.rs` 锁定诊断

### `unsafe fn(...)` 类型（`unsafe` 歧义消解）

`unsafe` 前缀三种形态正式区分：

| 形态                        | 语义                     | 示例                                       |
|-----------------------------|--------------------------|--------------------------------------------|
| 裸 `unsafe`（后跟 `^`/`-`） | unsafe impl 标记（不变） | `unsafe^T`                                 |
| `unsafe fn...`              | unsafe fn 类型（新增）   | `unsafe fn(u32)->u32`、`unsafe fn^(A,B)-C` |
| `unsafe X`（X 非 fn，并列） | 报错（忘写 `^` 的笔误）  | `unsafe Vec<u8>`                           |

- `TyFn` 新增第三字段 `is_unsafe`（types.rs），`apply_help`/`hoist_type_params`/
  `ToTokens` 透传；parse.rs 对 `TyPrefix::Unsafe` 特判：rest 以 `fn` 开头 → 置位，
  否则并列 → `compile_error!`
- 正确区分 `unsafe^fn(A)`（unsafe impl、目标为 fn 类型）与 `unsafe fn(A)`（unsafe fn 类型）：
  `^` 在 Caret 层切开，parse_primary 只看到裸 `unsafe`；无运算符时同时看到两个 ident
- 修复实测 bug：此前 `unsafe fn(u32)->u32` 被当作 unsafe impl 标记，且目标类型
  丢失 `unsafe`（生成 `for fn(u32)->u32`）
- 测试：`tests/dsl.rs` 第 29 节（三种写法 + 类型签名验证）、`tests/ui/unsafe_non_fn.rs` 锁定诊断

### 空操作数严格化（`^`/`-`/`,` 左右不能空）

实测发现 `^`/`-` 的**左空**此前是漏网的：`-A` 静默吞段（整个 spec 无声消失）、
`^A` 生成 ` <A>` 垃圾类型（下游报错定位不到 DSL）。现统一严格化：

- `-A`：parse.rs Dash 分支对 `parse_operand` 返回 None 但游标未到末尾（空段）报错；
  右侧空已由 0.5.3 覆盖
- `^A`：Caret 分支对首个操作数补 `is_empty_operand` 检查
- `,A`（前导逗号）：流式游标无法在 parse_item 内区分"前导逗号"与"上一个 spec 后的
  分隔逗号"，在知道调用序的 3 个入口判定：`batch_trait_entry` / `parse_list` /
  `parse_function`
- `A,,B`（连续逗号）：parse_item 逗号分支 bump 后若仍是逗号则报错
- 尾随逗号（`A,` / `[A, B,]`）、`()`/`[]` 等真实 token 不受影响；`;` 段落边界保持宽松
- 测试：`tests/ui/leading_operator.rs`、`tests/ui/leading_comma.rs` 锁定诊断，
  `tests/dsl.rs` 第 31 节锁定合法形态不受影响

### 文档漂移修复

README「元组生成」删除已过时的"u8 范围"（0.5.2 已改 usize）；测试矩阵计数更新为
当前值（dsl 31、regression 23、ui 20 个 compile_fail）；README 补充 `unsafe fn` 类型、
`#except` 指令、操作数严格性说明。

### 逻辑精简重构（行为零变化）

- **`Ty::expand` 包装样板压缩**（types.rs）：WithCode/WithWhere/WithAttr/WithPrefix
  四个 `Option` 内层包装与 WithType/WithTrait 两个非 Option 包装的"递归内层并重包"
  逻辑抽为 `expand_wrapped` / `expand_rebuild` 两个小辅助，六个臂各压到一行调用，
  types.rs 净 -27 行
- **指令展开骨架合并**（preprocess.rs）：`expand_fill` / `expand_delegate` 共用的
  "解析方法名列表 → 逐 item 构造 → 打包"循环抽为 `expand_many`，消除两份重复骨架
  （行数基本持平——被共享的是控制流，各指令的 body 构造本就不同）
- 全量测试通过，fuzz 全管线兜底

## 0.5.2 (2026-08-01)

### 解析器 fuzz 验证

新增 `src/fuzz.rs`（proptest，`cargo test --lib`）：用随机 token 序列
（覆盖 DSL 关键字、`^`/`-`/`,`/`;`/`::` 等运算符、括号嵌套深度 3）喂给最危险的
两个入口 `where_process` 与 `parse_item`，断言任意输入均不 panic —— 为
"不因用户输入 panic"的承诺提供属性测试背书。

### 发布卫生

- `#![forbid(unsafe_code)]`：库零 unsafe 变为硬约束
- `#![deny(missing_docs)]`：强制 `pub` 项文档（内部 `pub(crate)` 不受限）
- 修复 Windows MSVC 下无害的 `linker_messages` 告警（链接器 stdout 提示被 rustc 误报）

### CI 与文档

- 新增 GitHub Actions CI：`fmt --check` + `clippy --all-targets -- -D warnings` +
  `cargo test` + `cargo doc`，stable 与 MSRV（1.93.0）双工具链
- README 测试矩阵更新：新增 fuzz 层，regression 22、ui 10 个 `compile_fail`

### 数组/切片 builder（`TyPrimitiveArray`）

合并 `TySlice` 与 `TyFixedArray` 为 `TyPrimitiveArray(Option<Box<Ty>>, Option<TokenStream>)`，
`[]`（空基座）/ `[T]`（切片）/ `[T; N]`（定长数组）三种状态用 Option 表示：

- `[]^T` => `[T]`（空基座包出切片）
- `[T]^N` => `[T; N]`（定长数组；`N` 可为数字字面量、const 泛型标识符、范围或列表）
- `<const N: usize> []-X-N` => `[X; N]`：`[]` 作 `-` 累加链基座，把整个类型矩阵
  包进 const 泛型定长数组（如 `[]-[&, self, Box]^[u8, i8, ()^0..3]-N`）
- `()^N` 的 fresh 泛型元组作为泛型实参/数组元素时自动外提（`T^<A>X` => `<A>(T^X)`，
  嵌套 `WithType` 参数并入 impl 泛型），修复 `Box^()^N` 与矩阵嵌入的既有 bug
- `TyNum` / `TyRange` 由 `u8` 改为 `usize`（数组长度可更大）
- 测试：`tests/regression.rs` 新增第 19 节（`primitive_array_rules`）

## 0.5.1 (2026-07-31)

### 原生 `where{...}` 后缀

DSL 原生支持 `where{...}` 后缀形式，为生成的 impl 块添加 where 子句：

```rust
#[batch_impl(<T: Clone> Sortable<T> Vec<T> where{ T: Ord } { ... })]
trait Sortable<T>{  }
```

- `where{...}` 跟在目标类型之后（spec 末尾）
- 多个 `where{...}` 会合并

### 裸 `where 谓词 {代码块}` 新语法

`where` 后可裸写谓词，谓词后必须跟 `{...}` 代码块；三个接口
（`#[batch_impl]` / `#[batch_impl_only]` / `batch_trait!`）统一支持：

```rust
#[batch_impl(<A> <B> PairAB<A, B> (A, B) where A: Clone, B: Clone { ... })]
trait PairAB<A, B>{  }
```

- 新增 `where_process.rs` 预处理模块：在指令预处理之后、DSL 解析之前
  扫描深度 0 的裸 `where`，收集谓词直至首个 `{...}` 代码块，改写为旧式
  `where{谓词}` 后缀；解析层零改动
- 边界判定排除 `ident!{...}` 宏调用体（如 `where F: Fn(u32) -> m!{} { ... }`），
  尖括号内代码块（如 `<N = {5}>`）不计入
- 谓词区内的逗号不被 spec 切分；多个 `where` 段可依次书写
  （`where A where B`），等价旧式多 `where{...}`
- 裸 `where` 后缺少代码块报 `batch-impl: \`where\` 谓词后缺少代码块 {...}`
- 测试：`tests/dsl.rs` 新增 25-27，`tests/ui/where_missing_body.rs` 锁定缺 body 诊断

## 0.5.0 (2026-07-28)

### `#[batch_impl_only]` 外部 trait 路径前缀

`#[batch_impl_only]` 支持 `#path::to::Trait:` 路径前缀，用于为外部模块中定义的 trait 生成 impl：

```rust
#[batch_impl_only(#ext::mod::TraitName: usize, isize)]
trait TraitName {  }
```

- `#` + `Ident` + (`::` `Ident`)+ + `:` 形式起始时，路径作为外部 trait 路径
- 路径末尾标识符必须与本地 dummy trait 名一致（否则报 `compile_error!`）
- 新增 `try_parse_path_prefix` 状态机函数（`lib.rs`），要求至少一个 `::`
- `#[batch_impl]` 不支持此前缀（它输出本地 trait 定义，路径前缀无意义）

### `Spacing::Joint` 精确检查

多符号标点（`::`、`->`、`..`）的识别增加 `Spacing::Joint` 检查，防止相邻但不粘连的标点被误判为双字符运算符：

| 位置                                 | 检测目标        | 改动                                                |
|--------------------------------------|-----------------|-----------------------------------------------------|
| `scan_with`（`parse.rs`）            | `->` 箭头       | 检查 `-` 的 `Spacing::Joint`                        |
| `find_colon_at_depth0`（`parse.rs`） | `::` vs `:`     | 重写为检查左右相邻 `:` 的 `Spacing::Joint`          |
| `parse_range`（`parse.rs`）          | `..` / `..=`    | 检查 `first_dot` / `second_dot` 的 `Spacing::Joint` |
| `batch_trait!`（`lib.rs`）           | `::` 路径分隔符 | 检查 `p.spacing() == Spacing::Joint`                |

### Range 处理集中化

`Apply for Ty` 外层 match 新增 `Ty::Range` 分支（`apply.rs`），统一处理右侧 Range 展开。移除 `TyTuple::apply` 和 `TyGroup::apply` 中的重复 `Range` 分支。

`T^(1..3)` → `[T<1>, T<2>]`，`T<A>^(1..3)` → `[T<A,1>, T<A,2>]` 等 const generic range 展开自动生效。

### 模块级文档

所有源文件（`apply.rs`、`codegen.rs`、`diagnostic.rs`、`parse.rs`、`preprocess.rs`）新增 `//!` 模块级文档注释，描述模块职责与版本历史。

### 模块拆分

从 `parse.rs`、`apply.rs`、`types.rs`、`preprocess.rs` 中拆分出独立模块，降低单文件认知负担：

| 新模块                | 拆自           | 职责                                                     |
|-----------------------|----------------|----------------------------------------------------------|
| `scan.rs`             | `parse.rs`     | Cursor 游标 + scan_with / ScanMode / is_punct            |
| `parse_atom.rs`       | `parse.rs`     | 原子层解析（parse_attribute / parse_function / parse_group / parse_prefix / parse_range） |
| `generic.rs`          | `parse.rs`     | `<...>` 泛型解析（parse_generic / parse_angle_bracket_contents / matching_angle） |
| `types_render.rs`     | `types.rs`     | `ToTokens for Ty` + params_to_tokens 系列                |
| `apply_tuple.rs`      | `apply.rs`     | TyTuple / TyGroup / TyFn / TyCodeBlock / TyAttr / TyTypeParam 等的 Apply impl + tuple_pow / map_range |
| `batch_trait_entry.rs`| `lib.rs`       | BFS 展开并列列表 → 逐叶子 generate_impl 的共享驱动       |
| `path_prefix.rs`      | `lib.rs`       | `#Path::to::Trait:` 路径前缀状态机解析                   |
| `preprocess_helpers.rs` | `preprocess.rs` | build_from_item / get_trait_item / collect_call_args / parse_names_from_tokens |

拆分前后公共 API 与 DSL 语法不变。

### 测试

- `tests/dsl.rs`：新增测试 21（`where{...}` 后缀）、22（`where{...}` 后置）、23（`<A><B>T` 合并 + `where`）

## 0.4.2 (2026-07-27)

### `#name{body}` 支持 const / type 项

`#name{body}` 指令现在可以为 trait 中的 const 常量和 type 关联类型赋默认值，
不再局限于 fn 方法。`build_from_item` 根据 item 类型自动选择输出格式：

- `#CONST_NAME{value}` → `const CONST_NAME: Type = value;`
- `#TypeName{type_def}` → `type TypeName = type_def;`
- `#method_name{body}` → `fn method_name(签名) { body }`（不变）

### `#fill` 扩展与 `#all` 标记

- `#fill(args){body}` 不再限制为 Fn 项，可用于 fn + const + type
- `#all` 含义变更为所有 item（fn + const + type）
- 新增 `#all_methods`（仅 Fn）、`#all_constants`（仅 const）、`#all_types`（仅 type）
- `#delegate` 仍仅支持 Fn（委托本质是方法调用），传入非 Fn 项报 `compile_error!`
- `#fill` 传入非 Fn 项不再报错

### 错误处理

- `expand_delegate` 中的 `todo!("error")` 替换为 `compile_error!`，包含 trait 名和 item 名

### 文档

- 更新 preprocess.rs 注释：`#name{body}` 不再标注为"单方法简写"
- 更新 `get_trait_item` 错误信息：`"没有找到方法"` → `"没有找到 item"`
- 更新 README 指令系统章节：补充 const / type 示例和 `#all` 标记说明
- 新增 8 项 const / type / `#all` 指令测试（tests 37-44）

### 测试与示例重组

examples/ 原本堆放了 4 个文件 4700+ 行的 `assert_eq!` 测试，与"examples"语义不符。本版重组为三层：

- 删除 `examples/{tests.rs, ds_tests.rs, my_tests.rs, debug_tests.rs}` 共 ~4800 行
- 新增 `examples/quickstart.rs` —— 单文件可运行 demo（~250 行），14 段覆盖基础→复杂，`cargo run --example quickstart` 直接观察输出
- 新增 `tests/regression.rs` —— 16 个 `#[test]`，从原 `examples/tests.rs` 抽取高价值 corner case：嵌套 `>>`、路径类型、const 泛型、生命周期、dyn + Send、10 组 `batch_impl` vs `batch_trait!` 一致性
- `tests/dsl.rs` 保持不变（20 个 `#[test]`）
- README「测试」段重写为四列表格 + 三组运行指令

### 工程化重构

零功能变化的代码工程化与测试体系铺底。

- **命名**：`apply::trait Type` 重命名为 `trait Apply`，统一"运算符语义"与
  trait 名稱的语义。`parse.rs` 的 `use crate::apply::Type` 同步为 `Apply`。
  仅是 trait 名变更，对外行为不变。
- **格式锁定**：新增 `rustfmt.toml`（`edition=2024`、`max_width=75`、
  `fn_call_width=60`、`match_block_trailing_comma=true`、
  `use_field_init_shorthand=true`），要求 PR 通过 `cargo +nightly fmt --check`。
  仓库内一次性 `cargo +nightly fmt` 全量格式化。
- **诊断统一**：新增 `src/diagnostic.rs` 暴露唯一 `compile_error_str(msg)`
  构造器；删除 `lib.rs::generate_compile_error` 与 `preprocess.rs::compile_error`
  两份同名实现，防止诊断构造点漂移。未来若要引入带 `Span` 的诊断结构，
  只需改 `diagnostic.rs` 一处。
- **扫描器合并**：`parse.rs` 引入 `enum ScanMode { Lossy, Strict }` 与
  单一 `scan_with(tokens, stop, mode)`；`scan_stop`（宽松，
  用于停止符扫描）与 `matching_angle`（严格，
  用于尖括号配对）退化为两个对外语义别名，
  消除原先两份近似但行为不同的 `<>` 深度循环。
- **WithType 顺序修正**：`codegen.rs::extract_impl_parts` 的 `WithType`
  分支从 append 改为 prepend——
  `<A>[<B>T1, <C>T2]` 现输出 `impl<A, B>` 与 `impl<A, C>`，
  与"外层先写"的书写顺序一致。修复了同名泛型被反转的隐性问题。
- **错误加固**：`preprocess.rs::expand_tokens` 中两处
  `cursor.peek().unwrap()` 替换为 `let Some(tt) = cursor.peek() else { break; };`，
  彻底消除预处理层 panic 点；`apply.rs::tuple_pow` 单元素分支
  `.unwrap()` 改为带消息的 `expect`，保留不可达性追踪。
- **入口收敛**：`lib.rs` 内联 `extract_trait_path` /
  `extract_last_ident` 到 `batch_trait!` 宏内部，
  导出函数集中于 `diagnostic.rs`；`lib.rs` 由 303 行降到 ~276 行。
- **测试体系**：新增 `tests/dsl.rs` —— 20 个 `#[test]` 用例覆盖
  基础、泛型、共享/独立 body 合并、`^` 列表、元组生成、范围元组、
  关联类型、unsafe、fn 类型、属性、复杂透传、5 个 `#` 指令、
  `batch_trait!` 多段、`-` 操作符、嵌套泛型合并。
  新增 `tests/ui.rs` + 8 个 `compile_fail` UI fixture +
  1 个 pass fixture，通过 `trybuild` 锁定 DSL 错误诊断的中文措辞。
  重新生成快照：`TRYBUILD=overwrite cargo test --test ui`。
- **依赖**：新增 `[dev-dependencies] trybuild = "1.0.118"`。
- **文档**：README「内部架构」图加入 `diagnostic.rs` 与 `Apply trait` 名称。

## 0.4.1 (2026-07-25)
修复了自定义宏未携带trait_def问题

## 0.4.0 (2026-07-25)

### 指令系统

新增 `#` 指令系统，`#[batch_impl]` 在 DSL 解析前预处理指令，从 trait 定义自动读取方法签名。

| 指令   | 语法                      | 效果                                      |
|--------|---------------------------|-------------------------------------------|
| 单方法 | `#method{body}`           | `{fn method(签名) { body }}`              |
| 填充   | `#fill(args){body}`       | `{fn m1(sig){body} fn m2(sig){body} ...}` |
| 委托   | `#delegate(args){target}` | `{fn m1(sig){(target).m1(args)} ...}`     |

- `#fill(#all){body}` 表示 trait 的所有方法
- 指令与 DSL 运算符、`{body}` 连续附着、泛型、unsafe 等特性自由组合
- 仅 `#[batch_impl]` / `#[batch_impl_only]` 支持（`batch_trait!` 无 trait 定义，无法读取签名）
- 指令预处理错误输出 `compile_error!`（不 panic）

### 指令扩展性

内置指令（`#fill`、`#delegate`）由 batch-impl 内部处理。对于不认识的 `#name`，预处理器自动转换为 `#[name[...]]` 属性，用户的自定义属性宏可以接收并处理。这意味着 batch-impl 的指令系统是开放的——任何符合 `#name(...){...}` 语法的指令都会被预处理器捕获，不认识的名字委托给 Rust 的属性宏系统。

### `#[batch_impl_only]`

新增 `#[batch_impl_only]` 属性宏：与 `#[batch_impl]` 语法完全相同，但丢弃 trait 定义，只输出 `impl` 块。用于 trait 已在别处定义、只需批量生成 impl 的场景。

### `{body}` 连续附着

`T{body1}{body2}` 现在正确递归附着，等效于 `{body2}` 套在 `{body1}` 外面。

### 内部

- 新增 `preprocess.rs`：指令预处理模块，仅递归展开 `[...]`（Bracket）Group
- preprocess 的 `expand_tokens` / `expand_directive` 返回 `Result`，错误时输出 `compile_error!` 而非 panic
- 全库零 `panic!` / `unreachable!`：AST 层新增 `Ty::Error` 变体经 `ToTokens` 输出 `compile_error!`，预处理层 `parse_method_names_from_tokens` / `get_trait_method_sig` 返回 `Result`，错误沿调用链传播
- 新增 `examples/my_tests.rs`：36 项指令测试

---

## 0.3.0 (2026-07-24)

### 用更合理的框架重写了 batch-impl

v0.3.0 是从零开始的完全重写。公开 API 和 DSL 语法与 v0.2.x 保持一致，
内部实现与旧版本无任何代码上的联系。

### 架构

```
lib.rs            宏入口 + 共享驱动（#[batch_impl] / #[batch_impl_only] / batch_trait!）
  ├── preprocess.rs  指令预处理：#name 指令展开（内置 + 自定义属性委托）
  ├── parse.rs       DSL 解析器：Cursor 游标 + 优先级攀爬
  ├── types.rs       AST 节点（Ty 枚举 + 20 个变体）+ Op 优先级定义
  ├── apply.rs       运算符语义：apply() 折叠规则 + 元组展开
  └── codegen.rs     代码生成：Ty 递归拆解 → impl 块组装
```

**解析模型**：基于 `Cursor<'a>` 借用切片游标的优先级攀爬。四级运算符层级
`Semi(;)` < `Comma(,)` < `Dash(-)` < `Caret(^)`，每级定义一组停止字符，
`scan_stop` 统一处理 `<>` 深度跟踪与 `->` 箭头守卫。
操作数边界由词法级扫描确定（只看 `<>` 深度，不理解 Rust 类型文法），
任意 Rust 类型（`dyn Fn()`、`for<'a>` 等）透传为 Primitive 节点。

**AST 设计**：`Ty` 枚举含 20 个变体，分三类：
- 叶子（Primitive / Num / Range）：不可再展开的原子
- 包装（WithType / WithTrait / WithCode / WithAttr / Unsafe / Modified）：携带元数据，codegen 阶段拆解
- 容器（Array / Tuple / Group / Slice / FixedArray）：可展开为多个叶子的集合

**运算符语义**：`Type` trait 的 `apply(self, o: Ty) -> Ty` 方法定义二元运算。
`^` 右结合、`-` 左结合通过解析器的 `Caret` / `Dash` 分支实现；`[]` 并列列表
自动分发（`[A,B]^C = [A^C, B^C]`）；元组生成（`()^N`、笛卡尔积、范围语法）
在 `TyTuple::apply` 中实现。

### 功能

- `#[batch_impl]` 属性宏 + `batch_trait!` 函数式宏，接受相同的 DSL 语法
- `^`（右结合）/ `-`（左结合）运算符：泛型应用、类型组合
- `[A, B, C]` 并列列表 + `{ body }` 独立/共享实现体合并
- `<T: Clone, Item=V>` 泛型参数与关联类型绑定
- `()^N` 元组生成 + `(<Bound>)^N` 带约束元组 + `(T1,T2)^N` 笛卡尔积 + 范围语法
- `&` / `&mut` / `*const` / `*mut` / `fn` / `self` / `unsafe` / `#[attr]` 前缀修饰符
- `fn(A,B)->C` 函数类型
- `HashMap<K>^V` 预填泛型追加
- `unsafe^T` 单条 unsafe + `unsafe trait` 自动 unsafe
- `compile_error!` 错误输出（不 panic、不 ICE）
- 全量函数文档注释

### 修复（相对于 v0.2.x）

- `batch_trait!` 中 `fn(i32) -> bool` 等含 `->` 的 spec 不再误断段落边界
- `()^0` 正确生成空元组 `()`

### 测试

- 两套测试（tests 95+ 项 / ds_tests 56+ 项）全部通过
- clippy 零警告（lib）

---

## 0.2.2 (2026-07-20)

### Bug Fixes

- **fn^i32 自动生成括号**：`fn^i32` 现在正确生成 `fn(i32)` 而非 `fn i32`
- **统一 `->` 处理**：所有工具函数（`has_top_level_char`、`parse_balanced`、`find_top_level_colon`、`split_at_punct`）统一排除 `->` 中的 `>`

### 代码审查修复

#### P0 修复

- **split_raw 检测多余 `>`**：当 `>` 在 `<` 之前出现时报错（排除 `->` 的情况）
- **parse_balanced 详细错误**：返回 "未闭合的 `<`（还有 N 层）" 而非简单的 "未闭合的 `<`"
- **expand_caret 路径注释**：prefixes 为空时添加详细注释说明语义

#### P1 修复

- **expand_nested_bracket 注释**：添加 `unwrap_count - 1` 语义的详细说明
- **generate_tuples 返回 Result**：笛卡尔积超限时返回 `ParseResult::Err` 而非 `compile_error!` target
- **batch_trait! 空路径检查**：添加对空 trait 路径的显式检查和友好错误提示

## 0.2.1 (2026-07-20)

### Bug Fixes

#### 优先级修复：`^` 右侧 `-` 被内部消化 (BUG-1)

- **问题**：`HashMap^K-V` 被解析为 `HashMap^(K-V)` = `HashMap<K<V>>`，违反 `^` > `-` 优先级规则
- **修复**：`expand_caret` 中将右侧在第一个顶层 `-` 处分割，确保 `^` 优先级高于 `-`
- **结果**：`HashMap^K-V` = `(HashMap^K)-V` = `HashMap<K>-V` = `HashMap<K, V>`
- **注意**：`Box^Vec-u32` 是错误写法，应写为 `Box^Vec^u32`

#### `parse_target_items` 丢弃 `<>` 之后内容 (BUG-2)

- **问题**：`HashMap^<u32>-String` 中 `-String` 被静默丢弃
- **根因**：`parse_balanced` 返回的 `pos`（`>` 之后位置）被丢弃
- **修复**：当 `<>` 之后还有内容时，返回整个序列为 `Single`

#### `expand_single` 中 Attribute/Unsafe 前缀未过滤 (BUG-3)

- **问题**：`unsafe^#[attr]^T` 编译错误 "属性 ^ 的内部错误"
- **根因**：`expand_single` 未过滤 `Attribute`/`Unsafe` 前缀，直接传给 `apply_caret`
- **修复**：在调用 `apply_caret` 前过滤 `Attribute`/`Unsafe` 前缀

#### fn 类型优先级修复

- **问题**：`fn^(u32,i32)-usize` 生成 `fn(u32,i32,usize)` 而非 `fn(u32,i32)->usize`
- **修复**：`fn` 前缀应用后，`-` 应作为返回类型而非参数追加
- **结果**：`fn^(u32,i32)-usize` = `fn(u32,i32)->usize`

#### 嵌套 caret + fn 前缀修复

- **问题**：`fn^(u32,i32)^i64-usize` 中 `Fn` 前缀丢失
- **修复**：嵌套 caret 递归展开时，保留 `Fn` 前缀应用

### Code Quality

- 添加 `ImplSpec::new()` 构造器，消除重复的 `attributes: vec![]` 和 `is_unsafe: false` 初始化
- 拆分 `expand_caret` 中 bracket 展开逻辑为 `expand_bracket_with_comma` 和 `expand_nested_bracket`
- 拆分 `dash_append` 中 fn 处理逻辑为 `dash_append_fn_keyword` 和 `dash_append_fn_type`
- 添加 `#![allow(linker_messages)]` 抑制 Windows MSVC 链接器警告

## 0.2.0 (2026-07-19)

### 新功能

#### 关联类型简洁写法
- `TraitName<AssocType=value>` 语法：在 trait 泛型参数中指定关联类型绑定
- `<T> Iter<Item=T> Vec<T>` → 生成 `impl<T> Iter for Vec<T> { type Item = T; ... }`
- 支持多关联类型绑定：`Pair<First=T, Second=U>`
- 支持复杂类型绑定：`TupleAssoc<Output=(T, T)>`
- 关联类型可与 `^`、`-`、unsafe 任意组合

#### 独立/共享 body 合并
- `[A{bodyA}, B{bodyB}]{shared}` 语法：列表项可有独立 body，与共享 body 合并
- 共享 body 提供公共实现，独立 body 提供类型特定实现
- 合并策略：拼接（shared + independent）
- 支持多层嵌套：`[[A{...}, B{...}]{shared1}, C{...}]{shared2}`

#### 元组生成规则修改
- `()^N` → 生成带 N 个泛型参数的元组 `(A,B,...)`
- `(T)^N` → 生成长度为 N 的元组 `(T,T,...,T)`
- `(T1,T2)^N` → 生成长度为 N 的所有笛卡尔积组合
- 支持范围语法：`()^M..N` 和 `()^M..=N`

#### *const/*mut 指针支持
- `*const^T` → `*const T`
- `*mut^T` → `*mut T`
- 支持链式应用：`*const^Box^T` → `*const Box<T>`

#### 引用类修饰符特殊行为
- `&^A^B` → `&A<B>`（`&` 先绑定到 `A`，然后 `^B` 应用到结果）
- `&mut^A^B` → `&mut A<B>`
- `*const^A^B` → `*const A<B>`
- `*mut^A^B` → `*mut A<B>`

#### fn 关键字支持
- `fn^(A,B)` → `fn(A,B)`：fn 类型创建
- `fn(A,B)^T` → `fn(A,B)->T`：fn 类型追加返回类型
- `fn-(A,B)^N` → 生成 N 长度组合的 fn 类型

#### #[...] 属性支持
- `#[attr]^T` → 在 impl 块前添加属性
- `#[a]^[#[b]^B, #[c]^C]` → 生成带嵌套属性的 impl 块

#### 实现细节
- `ImplSpec` 新增 `assoc_bindings` 和 `attributes` 字段
- `PrefixItem` 新增 `ConstPtr`、`MutPtr`、`Fn`、`Attribute` 变体
- `parse_segment` 解析 `TraitName<Item=T>` 时分离关联类型绑定
- `expand_caret` 和 `expand_dash` 正确传递 `assoc_bindings`
- `generate_impl` 输出属性和关联类型绑定到 impl 块

#### 测试
- macro-test：113 个测试用例
- ds-test：15 个边界测试
- 新增测试：关联类型、`*const`、`*mut`、引用链式应用、fn 关键字、范围语法、属性支持
- 一致性测试：batch_impl 与 batch_trait 一致性验证
- 嵌套测试：多层嵌套 body 合并验证
- 并行测试：多功能并行使用验证

## 0.1.1 (2026-07-19)

### 新功能

#### 预填泛型追加
- `A<B>^C` → `A<B, C>`：容器带预填泛型时，`^` 追加参数而非生成 `A<B><C>`
- `HashMap<K>^V` → `HashMap<K, V>`：示例
- `[Box, Cow<'_>]^T` → `Box<T>, Cow<'_, T>`：列表支持
- `-` 运算符自动受益：`HashMap-u32-String` → `HashMap<u32, String>`

#### 实现细节
- 修改 `PrefixItem::Container` 结构体，增加 `prefill` 字段
- `parse_single_prefix` 支持识别 `Ident<...>` 模式
- `apply_caret` 支持预填泛型追加
- 新增 `append_to_generic_container` 函数处理 `-` 运算符

#### 文档更新
- README 添加优先级说明：`^` > `-` > `,`
- 函数注释补充预填泛型追加功能说明
- 移除 Planned 部分

#### 测试
- 新增 2 个测试用例验证预填泛型追加功能

## 0.1.0 (2026-07-19)

### 初始发布

#### 核心功能
- `#[batch_impl(...)]` 属性宏：为 trait 批量生成 impl 块
- `batch_trait!(...)` 函数式宏：对已声明的 trait 批量生成 impl

#### 运算符
- `^` 右结合运算符：泛型应用 `A^B` → `A<B>`
- `-` 左结合运算符：同 `^`，`A-B` → `A<B>`

#### 元组生成
- `()^N` 生成不同长度的元组实现
- `(<Bound>)^N` 生成带泛型约束的元组
- `(T1,T2)^N` 笛卡尔积生成
- `()^M..N` 和 `()^M..=N` 范围生成

#### 泛型支持
- impl 泛型：`<T>`, `<T: Clone>`, `<const N: usize>`
- trait 泛型：`TraitName<T>`
- 生命周期：`<'a, T: 'a>`
- 泛型继承：子项可省略泛型，自动继承父级

#### unsafe 支持
- `unsafe^T` 单条声明标记为 unsafe impl
- `unsafe trait` 全部 impl 自动 unsafe
- `batch_trait!(unsafe Trait: ...)` 部分 unsafe

#### 安全性
- 递归深度限制（128 层）
- 使用 `byte_range()` 生成稳定的位置后缀
- 笛卡尔积组合数上限（1024）

#### 错误处理
- 中文错误提示
- 保留原始 Span 信息
- `compile_error!` 而非 panic

#### 测试
- macro-test：99 个测试用例
- ds-test：15 个边界测试
- 覆盖：基础类型、泛型、元组、`^` 运算符、`-` 运算符、unsafe、特殊类型、关联类型、独立/共享 body
