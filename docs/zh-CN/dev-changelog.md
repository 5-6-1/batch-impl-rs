# 开发者变更记录

> 内部实现细节、重构、测试、CI；用户可见功能见 `CHANGELOG.md`。

## 0.7.1 (2026-08-10)

- **兜底校验**（`parse::generic::primitive`）：类型位置的 `;`/`=`/`@`/`#` 残留与相邻类型片段（`A B`/`Vec<T>U`/`[A B]`）定向报错——不再渲染非法 Rust；排除路径/range/泛型/fn/dyn/lifetime 名（不误伤 `Vec<u32>`/`a::b`/`0..3`/`dyn Trait`/`&'a T`）
- **`parse_function` 尾部**：fn 参数列表后残留 + `(<T: Bound>)` 元组生成器声明处理
- **blanket 返回 `Self`/`Self::Assoc` 拒绝**：朴素 `(**self)` 委托匹配不上包装的 `Self`——定向报错并建议 `#name{...}`
- **`MAX_NEST_DEPTH` 上移 util + `depth_err` 合并**：三处递归 walker 统一到 `util::MAX_NEST_DEPTH` + 统一构造诊断
- **`generate_impl` 拆分**（codegen/mod + where_at + impl_parts）：impl 泛型名/继承提取共用，行为等价
- **passthrough 一致性测试 + 探针转回归**：`bracket_is_passthrough` 四递归入口一致性 + 4 个新 ui fixture + adjacent_types

## 0.7.0 (2026-08-10)

### trait 泛型实参替换进指令 body + codegen 后处理层

- **新能力**：spec 级 trait 段带具体实参（`Conv<bool> [Pair<A, A>, Pair<B, B>] #conv{...}`）现在会把 trait 的泛型参数替换进指令抄写的 body——生成的 impl 里 `fn conv(value: T)` 变成 `fn conv(value: bool)`（此前裸 `T` 会泄漏进 impl，E0425）。`#[batch_impl]` 与 `#[batch_impl_only]` 都支持；trait 定义是参数名的来源。
- **codegen 后处理层**（`codegen/postprocess.rs`）：trait 泛型替换从 preprocess 移出（preprocess 不再通过 `expand_tokens`/`expand_directive`/`build_from_item` 穿参数映射），改为对 `ImplParts` 的后处理——把 `ImplParts::trait_generic_names`（具体实参）与入口 trait 的 type/const 参数名（经 `run_pipeline` → `parse_batch_trait_entry` → `generate_impl` 传递）配对，重写 body（fn 签名 + 用户代码块）。lifetime 实参（`'static`）与 lifetime 参数排除——body 引用的是自身 impl 的 lifetime。这与 `sweep_fresh_names` 一起构成"codegen 后处理"概念：提取之后、渲染之前的复杂 token 重写，`ImplParts` 携带全部所需上下文。
- 测试：`trait_generic_args`（dsl）——真实（非丢弃）trait 的泛型替换，验证 impl 编译通过且方法可引用；`trait_generic_args_to_impl_generic`——实参指向 impl 泛型（`<U>A<U>()` → `fn foo(_: U)`）。
- **已修复 edge（trait 段 + 右 splat）**：`Conv<bool> Pair^*(A, B)` 此前误解析成 `Pair<A<B>>`；splat 延迟展开重构（见下）让 `*(A,B)` 在 parse/apply 全程保持整体、仅在 codegen 展开——同一输入现在产出 `Pair<A, B>`（dsl `splat_scenarios` 的 `assert_cv::<Pair<SplatA, SplatB>>()` 验证）。数组 splat 替代 `Pair^[*(A),*(B)]^2` 仍照常工作。

### splat 展开延迟到 codegen（parse/apply/expand 全程保持 `*()`/`*[]` 整体）

- **原则（用户拍板）**：splat（`*(...)` / `*[...]`）在 parse/apply/expand 是**整体**——只在 codegen 后处理摊平成元素。此前 apply 层直接摊平右 splat 操作数（`T^*(A,B)` → 扁平 `T-A-B-...` 链），与 trait 段（`Conv<bool> Pair^*(A,B)` → `Pair<A<B>>`）和尾部代码块（`Pair^*(A,B) {body}` → rest 解析路径产出 `Pair<*const (A,B)>`）组合时误解析。
- **现在 splat 摊平的位置**（codegen 内单一展开点）：
  - `expand_splat_elems`（Ty 结构层）：`TyTuple` 内的 splat 元素摊平且 fresh 声明提升——`(A, *(B,C))` → `(A,B,C)`、`(*(()^3))` → `<P0,P1,P2>(P0,P1,P2)`。在 `hoist_type_params` 之前运行。
  - 泛型实参与 trait 实参的 splat 在同一趟经 `expand_tp` 摊平（TyTypeParam 的 params 现在是 `Box<Ty>`，splat 保持结构）：`T<*(A,B)>` → `T<A,B>`、`Map<*(K,V)>` → `Map<K,V>`（嵌套递归）、`Conv<*(A,B)> X` → `impl Conv<A,B> for X`（trait 路径 splat 在 `extract_impl_parts` 展开，即 trait 实参渲染处）。原先的 token 层 `expand_splats` 已删除——body 不经过任何展开器，fn 里的 `a * b` 保持乘法；`*const T` / `*mut T` 保持原始指针。
  - spec 列表位置的 splat（`[*(A),*(B)]`、`*[Vec,Box]^T`）仍在 expand 阶段摊平（`TyKind::Splat` → `Expand::Many`）——那是 impl 列表生成，不是类型结构展开。
  - 泛型实参里的 splat（`Foo<*(a,b)>`）不需要 parse 特例——chunk 走默认路径、作为单个 `*(a,b)` 实参存活、结构层经 `expand_tp` 展开——`Foo<*(a,b)>` → `Foo<a,b>`（专门的 Splat-arg 分支与 `contains_generator` 已删；ui `gen_splat_arg` 移除）。generator splat 在这里（`Foo<*(()^N)>` / `<*()^3>`）作为裸实参存活、由 rustc 报缺失声明——已知怪异，不做专门诊断。
  - **泛型实参里的 splat 幂**（`Frac<*(*@u*)^2>`）：幂结果（`TyArray([*(u8,u8), ...])`）进入 params 后，在 `expand` 的 Generic 分支分发成逐对 impl（36 个，与右 splat 链 `Frac^*(*@u*)^2` 等价）。**数组实参分发统一为单一路径**（用户原则"规则不通用就不是规则"落地）：字面 `T<[A,B]>`、常量 `T<@u*>`（展开成 `[u8,...]`）、幂结果全部进 params 成 `TyArray`，在 `expand` 的 Generic 分支统一分发——parse 层 `has_array_arg` 与 `split_arg_candidates` 已删（dsl `splat_pow_arg` 验证；`[[A,B],C]` 嵌套数组从"递归摊平到叶子"变为"一层分发"——与 splat 一层展开一致）。
  - **容器规则**（`parse_group`）：组内是孤立 splat 解析为容器、splat 作为一个元素保持——`(*(a,b))` = `( *(a,b) )`（元组）、`[*(a,b)]` = `[ *(a,b) ]`（数组）——splat 元素只在 codegen 展开（渲染结果 `(a, b)` / `[a, b]`），尾逗号形式与裸形式共用一条代码路径（`lone_splat` 门控 parse_list；原先按定界符分的 `TyKind::Splat` 特判分支已删）。`(a)` 保持透明组、`[a]` 是切片。
  - **具体类型实参拒绝 binding/bound**（用户拍板"有 `Item=u32` 就是 trait"）：`parse_angle_bracket_contents` 加 `allow_special` 门控——binding（`Item = u32`）与 bound（`T: Clone`）只属 trait 路径（`Conv<Item = u32> X`）与泛型声明（`<T: Clone> Foo`）；具体类型实参遇 `=`/`:` 报 targeted 错误（此前 bound 被静默丢弃、struct binding 渲染非法代码）。新增 `compile_error_ty`（类型位置无分号版——`compile_error!` 在泛型实参内带分号是语法错）。顺带修了两个潜伏问题：`scan_stop` 跳过 `..=`（range 的 `=` 不是 binding 分隔符——`Vec<@0..=2>` 此前被误判成 binding）；`@N..M` 范围引用在类型位置报 targeted 错误（where 谓词专用，ui `concrete_binding`/`concrete_bound`/`at_range_in_type` 更新）。
  - **where 谓词约束**：裸 splat 作谓词主体（`where{*(A,B): Trait}`）在 codegen 明确拒绝——谓词是约束不是参数列表，结构展开器会产出非法的 `A, B: Trait`。元组谓词（`(*(A,B)): Trait`）与谓词内部 splat（`X: Trait<*(A,B)>`）保持合法（ui `where_splat_bad`）。
- **splat 存续不变**：`Pair^[*(A),*(B)]^2` 仍重复每个元素（`[Pair<A,A>, Pair<B,B>]`）；splat 幂（`*(A,B)^2` 笛卡尔积）与左 splat 追加/分配（`*[...]^T`、`*(...)^T`）在 `TySplat::apply_help` 照常。
- `TySplat::Tuple` 渲染改为 `*(A,B)`（原 `(*(A,B))`）——外括号只服务于旧的 parse 时消费；codegen 展开器匹配裸标记。
- **`<>` 内的 generator 实参**：`flat_splat_params`（共享 splat 摊平器）现在也提升 `WithType`（fresh generator）实参——`()^N` 保持内层元组为单个实参（`T<()^2>` = `impl<P0,P1> T for T<(P0,P1)>`）、splat 重包（`*()^N`）则摊平（`T<*()^2>` = `impl<P0,P1> T for T<P0,P1>`）。此前 `Pair<()^2>` / `Pair<*()^2>` 把声明漏进实参导致编译失败。测试：dsl `gen_args_in_angle`。
- **`TyTypeParam` 全面 Ty 化**：`params` 改为 `Vec<(Box<Ty>, Option<Ty>)>`、`bindings` 改为 `Vec<(Box<Ty>, Box<Ty>)>`——每个元素都是 `Ty`，非类型 token（参数名、`const N`、生命周期、数字 const 实参、绑定名）统一用 `TyPrimitive` 包裹。泛型实参因此结构化：`T<Map<K,V>>` 保持 `TyGeneric(T, [TyGeneric(Map, [K,V])])`、splat 实参（`T<*(A,B)>`）以 `TySplat` 存活并在 codegen（`expand_tp`）摊平、`@N` 仍在 parse 前解析。渲染/提取/apply 对 params 统一按结构化类型处理；声明与实参的区分仍在所用的渲染函数（`params_to_tokens` vs `params_to_tokens_no_base`）。
- 删除 `consume_splats`（parse 时摊平 splat 的 `parse_group` 逻辑）；`(a, *(b,c))` 与 `(*(a,b))` 现在保持 splat 直到 codegen。
- 测试：现有 splat 套件（SplatArgs / SplatConcat / SplatGen / SplatGenFlat / SplatSurvival / SplatLeft / 尾逗号 / 中间空 / 幂等）全部原样通过；新增 dsl `SplatGenericArg`（`SplatMap<*(A,B)>` → `SplatMap<A,B>`）与 `assert_cv`（trait 段 + 右 splat）覆盖延迟展开路径。
- **splat 存续（数组元素）**：数组/列表元素若是 splat，现在**保持到消费**而非 parse 时摊平（`parse_atom.rs` 不再对 `[...]` 列表或孤立 `[*(...)]` 元素调 `consume_splats`）——splat 活到 apply 右操作数或 codegen，所以 `[*(A),*(B)]^2` 会重复每个元素（`[*(A,A),*(B,B)]`），`Pair^[*(SplatA),*(SplatB)]^2` = `[Pair<SplatA,SplatA>, Pair<SplatB,SplatB>]`（splat 幂驱动两个泛型位）。裸数组/切片（`[u8]`、`[u8; 3]`）与无右操作数的目标（`[a, *[b,c]]` = `[a,b,c]`）不变（codegen 末尾摊平）。**有右操作数时**，保持的 splat 元素走自身 splat 语义（与独立 splat 一致）：`[A,B,C]^D` = `[A^D, B^D, C^D]`（裸列表：分发）、`[A,*(B,C)]^D` = `[A^D, *(B,C,D)]` = `[A^D, B, C, D]`（元组 splat：追加）、`[A,*[B,C]]^D` = `[A^D, *[B^D,C^D]]` = `[A^D, B^D, C^D]`（数组 splat：分发）、`[*(A)]^2` = `[*(A,A)]` = `[A, A]`（幂：重复）。要纯分发请写裸列表 `[A,B,C]^D`。元组仍在 parse 时摊平 splat（本次范围外）。测试：dsl `SplatSurvival`。
- **不诊断（刻意）**：fn 泛型参数与被替换的 trait 实参重名（`impl<U> A<U>` 里的 `fn foo<U>(_: T)`）是 Rust 自身的泛型遮蔽禁令——`E0403` 已经同时指向两个 `U`（spec 的 `<U>` 与 fn 的 `<U>`）。用户改名后宏输出合法代码；不加后处理检查（语言级规则，rustc 的诊断已足够精确）。

### 核心重组：codegen 拆分 + fresh 名协议统一

- `codegen/` 从 672 行单体拆为四个文件，全部在单文件预算内：`mod.rs`
  （generate_impl + 组装，242 行）、`top_level.rs`（顶层宏注入——spec
  主体合并 + 宏输入重写）、`fresh.rs`（fresh 名清扫）、`where_at.rs`
  （`@` where 谓词解析）；
- **fresh 名协议统一**到 `ast/fresh.rs`：保留模式 `_Param_*_BatchGen_`
  （前缀/后缀常量）+ 生成/构造/解析三函数——`fresh_param`（apply 层铸造
  `_Param_{g}_{i}_`）、`at_ref_name`（parse 层把 `@N`/`@g_i` 转为名字）、
  `parse_grouped_fresh`（codegen 层识别分组形式）——此前散落在
  `ast/types.rs`、`parse/mod.rs`、`codegen/mod.rs` 三处；三层现在共享
  单一协议源，不可能漂移。

### splat（`*` 前缀）

- 新增 `*[...]` / `*(...)` splat：把容器/生成器展开拼入外层列表——元组/数组内拼接
  （`[a, *[d,e,f]]` = `[a,d,e,f]`）、`^`/`-` 右操作数扁平追加（`T^*(A,B)` ≡ `T-A-B`）、
  泛型实参多实参（`Foo<*(a,b)>` = `Foo<a,b>`）、嵌套幂等（`*(*[a,b])` = `[a,b]`）、
  空无操作（`[a, *()]` = `[a]`）；
- **左操作数按来源分语义**：`TySplat` 是镜像 parse 定界符的枚举——`TySplat::Array`
  分配 `^T`（`*[A^T,B^T]`——集合，对标 `TyArray`，包回以便右 splat 链摊平进容器）、`TySplat::Tuple` 追加
  （`*(A,B,...,T)`——列表，对标 `TyTuple`，包回）；**splat 上的 `^N` 幂把每个笛卡尔组合包回
  splat**：`*(A,B)^2` = `[*(A,A),*(A,B),*(B,A),*(B,B)]`——每个组合是参数位列表，右 splat 链摊平进
  容器（`A^*(*@u*)^2` = `A<u8,u8>`/`A<u8,u16>`/...——`A<@u*,@u*>` 的重复列表简写；`*(A,B)^2`
  单独作目标摊平成重复，E0119——元组 impl 用 `(A,B)^2`）；**splat 只展开一层**：元组是类型保持
  （`*((a,b),)` = 一个 `(a,b)` impl；`*(a,(b,c))` 保持 `(b,c)`），数组/嵌套 splat/生成器/组摊平；
  `*()^N`（空 splat）把 fresh 元组包回 splat 以便载体追加参数（`T^*()^2` = `<A,B>T<A,B>`；裸 `*()^N` 单独作目标会被
  rustc 拒绝——多个 impl 共享一份泛型声明但各自只用其中一个参数，E0207）；裸 `*()` 单独作目标产生**零
  impl**（空列表无元素）；左操作数
  `apply_help` **委托 `TyArray`/`TyTuple::apply_help`** 再把结果包回对应 splat 变体
  （splat 在消费前保持 splat）——无重复的分发/追加逻辑。右操作数与容器收集
  一律摊平、与变体无关（用户决策——"`*` 意义就在这"）；
- `*const`/`*mut` 原始指针不受影响（按后续 token 区分）；裸 `*u8` 报定向错误
  （ui: `star_misuse`）；生成器 splat 作泛型实参报错——fresh 声明无处安放
  （ui: `gen_splat_arg`）；
- 右 splat 分支从三个 match 臂（元组拼接 / 生成器递归 / 转链）收敛为一条扁平转链——
  元组走自身 apply_help 拼接、生成器经 `TyWithType::apply_help` 递归且保留声明
  （此前版本手动解包 `*wt.1` 丢掉 decl——E0425）。

### 风格统一 + Apply trait 化

- 全部 `Ty*` 子类型实现 `Apply` trait（17 个）——`apply_help` 变为 trait 方法；
  内部递归统一调 `.apply()`（完整右操作数分发），绝不直接调 `apply_help`。
  `TySplat::apply_help` 现在是纯委托：`TySplat::Array(a) => a.apply(o)` /
  `TySplat::Tuple(t) => t.apply(o)`，再把返回的容器包回（splat 保持到消费）；
  `*()^N` 经 `WithType` 透传保持 splat 形态（`*()^2` = `<A,B>*(A,B)`）。
- 构造风格统一：`Ty::new(span, TyKind::X(sub))` 嵌套包装全库清除（`Ty::new`
  删除——49 处调用点改 `X(...).to_ty().with_span(...)`；透传用
  `Ty { span, kind }`）；值处包装用 `val.into()`（`Some(Box::new(x))` →
  `x.into()`——`From<Ty> for Option<Box<Ty>>`；唯一保留的 `Box::new` 在
  `From<$t> for Box<Ty>` 定义处——Box 唯一构造方式，裸 `value.into()` 会递归
  到 impl 自身（实测栈溢出））；类型标注移到等式右侧
  （`collect::<Vec<T>>()`、`parse::<usize>()`、`s.parse::<TS2>()`），必要标注
  （空 `vec![]`、`parse_quote!`）保留。
- parse 层拆分：`parse/mod.rs`（582 行）→ `chain.rs`（运算符攀爬）/ `primary.rs`
  （原子、泛型实参、splat）/ `trailing.rs`（body/where 拆分、wrapper 附着）+
  `mod.rs`（119）；parse 全部文件 ≤ 350。
- 文档：tutorial 强调 `#fill` 单元素推荐（`#name{body}` 优于 `#fill(name){body}`——
  全库核实无单元素 `#fill`）；splat 位置盘点：8 类允许位置、4 处宏定向错误
  （指令参数 / `@` 定义 / 裸 `*` / 生成器实参）、2 处 rustc 兜底
  （where 谓词 / 泛型声明）。

## 0.6.7 (2026-08-08)

### fresh 系统重构：分组生成 + 每 impl 清扫；`@N` 纯构造

- **分组 fresh 生成**：fresh 参数生成为 `_Param_{g}_{i}_BatchGen_`
  （g = spec 内的生成器序号——每 spec/段重置、DSL-local；i = 生成器内部
  位次）。codegen **清扫器**在渲染前把每个 impl 的 fresh 参数按
  (组次, 位次) 序重编号为 `_Param_0..N_BatchGen_`——即目标类型文档序；
- **每 impl 独立编号修复单元漂移**：每个 impl 独立清扫，`@N` 恒指
  *本 impl* 的第 N 个 fresh——跨 spec 与 range 生成可用
  （`()^1..=3 where{@0: Clone}` 与 `(()^2, ()^2 where{@0: Clone})`
  此前在后续单元报 "out of range"——计数器跨单元延续；现在每单元从 0
  开始）。`@N` 是纯构造（`@N` → `_Param_{N}_BatchGen_`）——无需查表，
  恒与清扫后名字匹配；
- **组合场景**（`()^3-()^3`）：`@0` 是左元组首元素（文档序——此前是
  声明序第一个——hoist 先声明嵌套元组——Breaking）；
- **目标类型 `@N` 通道**：`@N` 位置引用在类型域边界消解为 fresh 名
  （`parse_operand` + 尖括号扁平 chunk 的 `resolve_at_refs`——`Box<@0>`
  可用）。blanket 不再自行替换包装的 `@0` 位置标记（`replace_at0` 删除；
  `has_at0` 只保留位置决策——有 `@0` 原样发射、parse 层消解标记）；
- **声明序 = 文档序**：apply 双方都带泛型声明时（fresh-fresh 链如
  `()^3-()^3`）params 左先合并，hoist 按目标类型文档序收集；inner 只取
  左的 inner 部分（否则 hoist 重复收集左声明——E0403）；
- 标记占位方案评估（决策）：曾考虑"泛型名占位、codegen 统一生成"——
  否决：分组名 + 每 impl 清扫即达可预测，无需标记 token 系统（其需
  标记/`@N` 区分规则、parse 期替换、Ty 树 token 存活）；
- **`@g_i` 分组引用启用**：`@0_1`（含下划线的 Literal）在目标类型（parse
  层）与 where 谓词（impl 组匹配——impl 无该组时报 "no group g position i"）
  中解析为分组名 `_Param_{g}_{i}_BatchGen_`——跨数组分发 impl 稳定（`@N`
  在分发下文档序语义漂移）；清扫器把引用与生成名一起重编号；
- **`@N` 稳定性承诺**：编号语义在 0.6.4 → 0.6.7 间修订过三次（泛型参数族
  时代 → `@N` 语义修正 → 每 impl 编号 + 文档序 + 目标类型通道）。现机制
  （每 impl 清扫为 `_Param_0..N_BatchGen_`、`@N` 纯构造）视为**最终形态**
  ——今后任何改动都是刻意的破坏性发布。

### preprocess 目录重构 + 文档整理

- `preprocess/` 重组为两个子文件夹：**`directives/`**（`#` 指令系统：
  name_list / trait_items / delegate_args / blanket / blanket_wrappers +
  mod.rs 入口）与 **`consts/`**（`@` 常量系统：table / expand / ctx +
  mod.rs 入口）——扁平 12 文件混杂了两个无关关注点；`preprocess/mod.rs`
  收敛为 5 个 mod + glob re-export；内部路径更新
  （`crate::preprocess::consts::ctx::ConstCtx` 等）；
- tutorial：`@` 宏元层以**三个维度**（常量 / 选择器 / 位置引用）引入并加
  组合说明；新增 `@N` vs `@g_i` 选择指引、`@N` 稳定性承诺、学习成本提示
  （日常用 `@u*` / `@all_methods` / `@0` 即可；分组/批量/范围引用仅在
  谓词需要指名特定 fresh 时再学）；
- README：开头补分层定位——"带可插拔 codegen 协议的批量 impl 生成器"
  （"一行"故事之下还有宏元层 + 开放指令系统）；
- architecture 模块图同步新 preprocess 布局。
- 新增 dsl 测试：at_refs_numbered_match_in_join（u8-only Marker 约束验证 @0 = 文档序第一个 fresh）与 at_refs_across_generation_units（range 长度 + 多 spec）。
### 顶层宏注入（`{! ...}`）

- **开放扩展改为顶层**：`#cmd(args){body}` 展开为 `{ ! name!{(args){body} trait_def} }`
  ——`!` 标记顶层发射：codegen 剥离 `!`、把 spec 主体（目标类型，渲染为一个
  Brace 组）插到宏输入开头（4 段协议 `{spec}(args){body}trait`）、顶层输出
  宏调用——用户宏生成任意 item（通常是自己完整的 impl）；此模式下
  batch-impl 不再生成 impl（**Breaking**：开放扩展宏必须解析 4 段并生成
  完整 item，而非关联项）；
- **`T {! m!{...}}` attach 形态**：同一顶层协议、用户手写宏输入；
  `T {m!{...}}`（无 `!`）保留旧的内嵌形态（关联项——用户写完整输入含
  trait）。每个 spec 至多一个 `{!}` 且必须是最后一个块——`{!}` 后跟
  `{...}` 块报错（现状块序下或在 walk_top_level 的 "must be the last
  block" 报错、或经顶层路径在 rustc 层报错；ui fixture
  `top_level_block_not_last` / `top_level_manual_not_last`）；
- **守卫**：无 attach 类型的独立 `{! ...}` 报 "needs an attached type"
  （不再输出无效 Rust；ui fixture `top_level_without_attach`）；
  `{! }` 后无宏调用报 "must contain a macro call"；
  `walk_top_level` 区分"普通块内部发现 `{!}`"与"`{!}` 在更外层"，未来块序
  变更不会误报前置块；
- `batch_preprocess_test`（测试宏）支持双协议：4 段顶层形态生成
  `impl Trait for {spec}`；3 段内嵌形态生成关联 fn 定义。

### `@all_fresh` / `@N..M` 批量引用（where 谓词）

- `@all_fresh: Bound` 展开为每个 fresh 泛型一个谓词
  （`_Param_0_: Bound, _Param_1_: Bound, ...`）；impl 无 fresh 泛型或展开
  超过 `MAX_EXPAND` 时报错；
- `@N..M` / `@N..=M` 展开连续 fresh 段（`@0..=2: Clone` 约束前三个）；
  越界与超 MAX_EXPAND 谓词报错，空范围（`@0..0`）也报错（不再静默展开为
  空）；常量阶段放行 `@all_fresh`（where 专用选择器）；
- 类型内的范围引用（`Vec<@0..=2>`）在 parse 层报定向错误（ui fixture
  `at_range_in_type`）。

### 错误聚合

- driver 现在收集每个 spec 的错误（经 `map_children` 递归进嵌套包装——
  `Box<@0..=2>` 的错误在类型参数里）一次全部报出；旧行为停在第一个错误。
  有任何错误时只输出错误——不输出部分 impl（ui fixture
  `error_aggregation`）。

## 0.6.6 (2026-08-07)

### 元组/fn 语法边界修正（`(T)^2 = T^2`）

- `(T)^2 = T^2` 规则确认：分组剥离后幂 = const 泛型实参（`(u8)^2 = u8<2>`），
  TyGroup 恢复剥离语义（元组生成须 `(T,)^N`）；
- `(<T>)^2` 报错锁定（`(` 后 `<` 不是合法类型）——ui fixture group_angle_bare；
- 数字/范围渲染改 unsuffixed literal：`u8<2>` / `[u8; 3]`
  （原 `u8<2usize>` / `[u8; 3usize]`）；
- 教程修正：fn-arrow 说明（`fn(A,B)-C = fn(A,B)->C`，`->` 不是 DSL 操作符）、
  元组注意块（`(T)` 分组非元组 / `(<u8>)` 错误语法 / `(<Clone>)^N` 不支持）。

### 输入校验四连（评测员发现）

- `expand_consts` 补深度守卫（128 层，与 angle_collect 一致）：超深嵌套
  `[[[...]]]` 之前直接栈溢出（4000 层复现），现在报优雅的
  "nesting depth exceeds 128 levels"；
- `check_value_refs`（校验常量值引用的兄弟递归）补 128 层深度守卫——
  深嵌套常量值不再栈溢出（ui fixture const_value_deep_nesting）；
- `#blanket` 的 `:N` 深度加上限 128：`Box:999999` 之前生成百万级 `*`
  委托体导致 rustc 栈溢出，现在报 "deref depth must be ≤ 128"；
- batch_trait! 自定义常量在定义处拦截保留名：`@all_*` 前缀
  （`@all_methods = ...` 之前定义处通过、使用时才报错，现在定义处直接报
  "reserved `@all_*` selector"）与裸 `@all`；
- `#blanket` 冒号后空内容（`Box:`）之前静默通过（`:` 泄漏进类型、
  rustc 层报错），现在 DSL 层报 "after `:` must come a number"；
- 新增 ui fixture ×5：const_reserved_all / blanket_bad_empty_depth /
  blanket_bad_huge_depth / nested_bracket_too_deep /
  const_value_deep_nesting。

### #delegate 参数模式修正（评测员发现）

- **模式保留 + 表达式重建**：非 `_` 参数模式（如 `(a, b)`）保留原签名，
  委托调用直接用模式的 token 作表达式——`(a, b)` 解构绑定 `a`/`b` 后
  由 `(a, b)` 重建元组传给目标（`[x, y]` / `Foo { x }` / `&x` 同理）；
- **不可作表达式模式递归检测（pat_is_forwardable）**：`ref x`（by_ref）、
  守卫/`x @ pat`（subpat）、`_`、嵌套形式（如 `(ref x, ref y)`）都会
  递归查出并 fallback 改写成 `arg0`…（签名 + 调用同步，syn::Pat::parse_single
  解析）；可作模式（`(a, b)` / `[x, y]` / `Foo { x }` / `&x`）保留签名，
  委托调用直接以模式 token 作表达式重建；
- `collect_call_args` 返回 `Vec<TokenStream>`：Ident → 名字、可作模式 →
  `quote!(#pat)` 直接作表达式；
- `build_from_item_sig`：签名覆写变体（fallback 改名后需同步到生成签名）；
- 新增 dsl 测试 delegate_wildcard_param / delegate_tuple_pattern /
  delegate_ref_nested_pattern；
- 删除过时 ui fixture delegate_pattern_arg（delegate 不再拒绝模式参数）。

### 深度守卫加固（评测员洞 E / 洞 D）

- **守卫前移**：Group 递归前先检查 `depth + 1`（在 stream()/collect 之前
  拦截）——consts.rs / consts_expand.rs 两处，防"守卫在深拷贝后执行"；
- **实测澄清**：20000 层嵌套默认栈崩溃发生在 **rustc 解析宏参数阶段**
  （宏函数第一行未执行——entry trace 验证 `[entry] start` 未打印）；
  RUST_MIN_STACK 让解析通过后，守卫 128 层正常优雅拦截。proc-macro2
  的 Group Clone 是 Rc 共享（浅拷贝），"into_iter 深拷贝整棵树"的
  机制不成立——超深输入的上限是 rustc 宏线程栈，宏侧无法拦截；
- **Pat::Type**（类型注解模式 `x: u32`）不可作表达式 → fallback
  命名（洞 D，`(x : u32)` 不是合法表达式）。

### #blanket 包装的 `@0` 位置标记

- 包装主部分（去掉 where / :N）**带 `@0`** → `@0` 即目标 T 的位置占位，
  展开为原样（`@0` 替换成 fresh 泛型名）——T 可放任意位置
  （`(u32, @0)` → `(u32, T)`）；
- **不带 `@0`** → `部分^T`（T 附加末尾，现状不变）；
- has_at0 / replace_at0 helper（递归进组）；dsl 测试 blanket_at0_position /
  blanket_at0_const_generic（自定义 Deref + `<const N: usize>` 泛型，
  与用户参数 N 共存）。

### 指令文档占位宏族

- 新增 6 个 `#[proc_macro]` 空占位宏（doc 只读符号）：batch_impl_delegate /
  batch_impl_fill / batch_impl_blanket / batch_impl_name（`#name{body}`
  按名填充）/ batch_impl_open（开放扩展协议）/ batch_impl_consts
  （`@` 常量系统）——指令与常量的文档从"宏参数里的不可达 token"变成
  "可 hover / 可搜索的 rustdoc 符号"；
- **实测**：proc-macro crate 不能导出普通 `pub fn`（E0753——"cannot export
  any items other than proc-macro tagged"），占位用空 `#[proc_macro]`
  宏实现（`batch_impl_delegate!{}` 空展开无害，doc 正常渲染）；
- 每个占位带可编译 doctest 例子（57 个 doctest 全过）。

## 0.6.5 (2026-08-06)


### `#cmd[args]{body}` 等价写法 + blanket `@0` 统一到 codegen

- `#cmd[args]{body}`（方括号参数）确认可用并宣传：与 `(args)` 等价
  （`_` 分支通吃）；错误消息更新为 `(args)` or `[args]`；tutorial 指令章节
  注明两种写法（方括号在参数含括号时更清晰）；ui fixture
  `directive_bad_follow` 快照重新生成；
- **blanket 的 `@0` 统一到 codegen**：`resolve_target_predicates` 删掉 @0
  替换分支——`@0`/`@N` 保留原样进 spec，由 codegen 的 `resolve_where_at`
  统一解析（blanket 的 fresh 泛型是唯一 fresh，`@0` 索引正确）；预处理器
  只保留 `@trait` 替换（trait 路径只有预处理知道）；架构上"`@N` 是唯一
  codegen 记号"对 blanket wrapper where 也成立；
- 验证：fmt/clippy -D warnings 干净，lib 10 / dsl 51 / regression 26 /
  ui 34 fixture / doctest 50 全绿。
### punct 工具统一 + 指令系统打扫（#blanket 拆分）

- 库内 punct 工具（`is_punct` 既有 + 新增 `is_punct_at` / `is_joint_punct_at`）：
  替换散落的 `matches!(...Punct(p)...)` 独立表达式（consts/consts_expand/
  path_prefix/where_process/scan 内部），match 臂 guard / slice 解构 /
  绑定需求处保留（模式本身）；
- `#blanket` 拆分：blanket.rs 401 → `blanket.rs`（249：doc + expand_blanket +
  resolve_target_predicates + 新 trait_with_args）+ 新 `blanket_wrappers.rs`
  （160：BlanketWrapper + parse_blanket_wrappers）——全 ≤350；
- blanket.rs 的 t_bound/trait_part 重复收敛为 `trait_with_args`（trait 路径 +
  手动角度组，blanket 输出不再被 angle_collect 配对）；
- 验证：fmt/clippy -D warnings 干净，lib 10 / dsl 51 / regression 26 /
  ui 33 fixture / doctest 50 全绿。
### 宏调用 passthrough 洞修复（expand_consts + angle_collect 的 `()` 组）

- **洞**：`expand_consts` 与 `angle_collect_at` 的 `()` 组无条件进入递归——
  `ident!(...)` 宏调用的参数（用户 Rust）会被 DSL 常量替换 / `<` 被错误配对；
  此前只对 `[]` 组有 `bracket_is_passthrough` 守卫（`ident![...]` / `#[...]`），
  `()` 组漏了；
- 修：`()` 组统一走 `bracket_is_passthrough`（前 token 是 `!`/`#` 则原样保留）——
  宏调用 `foo!(...)` passthrough；`#name(...)` 指令参数（前是 Ident）与 DSL 元组
  `(A, B)` 仍进入；
- `render_angles` 同步：改为索引遍历 + passthrough 判定（宏调用组不重建、
  span 原样保留）；`angle_collect_at` 的 depth 错误 `map_or_else` 简化；
- 探针实测：`echo!(@u*)` 宏参数 `@u*` 原样传入（stringify = "@u*"）；
  angle 测试新增 `m!(a < b)`（Paren 宏调用含 `<`，不报错 + roundtrip 保持）；
- 验证：fmt/clippy -D warnings 干净，lib 10 / dsl 51 / regression 26 /
  ui 33 fixture / doctest 50 全绿。
### 常量系统打扫（consts 拆分 + 行为收紧）

- 拆分：consts.rs 520 行 → `consts.rs`（272：模块 doc + 内置常量表 +
  expand_consts 入口 + collect_user_consts）+ 新 `consts_expand.rs`（258：
  try_expand_at + check_value_refs）——依赖单向（consts → consts_expand），
  全部 ≤350；
- `render_list` / `render_list_strings` 合并为泛型 `render_list<S: ToString>`
  （&str/String 都支持，省一份重复）；
- try_expand_at 的 `tokens.first().map(...).unwrap_or_else(call_site)` 两处
  收敛为 `tokens[0].span()`（tokens[0] 恒为 `@`）；
- **行为收紧**：`check_value_refs` 的 known 判断加 `is_range` 条件——裸
  范围端点引用 `@a=@u8`（无 `..`）现在**定义处**报错（此前放行、使用处才炸）；
  新 ui fixture `const_bare_endpoint` 锁定；
- 验证：fmt/clippy -D warnings 干净，lib 10 / dsl 51 / regression 26 /
  ui 33 fixture / doctest 50 全绿。
### 构造链重构：`From<TyKind>` + `to_ty()` + `with_span` 取代 `TyKind::X(TyX(…)` 嵌套

- 作者指出：`impl_from_for_ty!` 宏的 `From<$struct> for Ty` 是 span 改造的遗留
  错位——宏本意是"子类型 → 变体"（span 前的 `Ty` = 现在的 `TyKind`）；
- 宏定稿四件套：`From<TyKind>`（纯结构转换，`TyArray(x).into()` 取代
  `TyKind::Array(TyArray(x))`）、`From<Ty>`（call_site 版，`to_ty` 的实现基础）、
  `to_ty()`（链式入口，显式返回类型解决 `.into().with_span(span)` 的 E0282——
  method resolution 无法反推 `.into()` 目标）、`From<Box<Ty>>`（Expand 遍历器用）；
  删 Option<Ty>/Option<Box<Ty>> 两个无调用点的 From；
- 新增 `Ty::with_span(span)`（只改节点层 span）；`Ty::new(span, x.into())` 与
  `x.to_ty().with_span(span)` 两种构造形态并存（各司其职）；
- 全库替换 ~50 处 `TyKind::X(TyX(…))` → `TyX(…).into()`（TyKind 目标）或
  `TyX(…).to_ty().with_span(span)`（Ty::new 内）；3 处模式位置（match 臂 /
  if let 解构）保留；
- net -74 行（+171/-245）；to_ty 消费 self 是刻意设计（clippy allow）；
- 验证：fmt/clippy -D warnings 干净，lib 10 / dsl 51 / regression 26 / ui 32
  fixture / doctest 50 全绿。
### Cursor 定位收敛（方案 A：parse 专属只读游标）

- 定位：`Cursor` = **parse 层专属**只读游标（entry/parse_atom/driver/generic/
  fuzz）；**预处理层（preprocess/*）统一 Vec+index 遍历**（重写语义，读改写）；
- 改动：`expand_tokens` / `expand_directive`（preprocess/mod.rs）改
  `tokens: &[TokenTree] + i`——expand_directive 返回 `(输出, 消费数)`；
  `where_process` 签名改 `tokens: &[TokenTree]`（入口不再包 Cursor）；
  `Cursor::peek_at` / `prev_bracket_passthrough` 删（无调用者）；
- 验证：fmt/clippy -D warnings 干净，lib 10 / dsl 51 / regression 26 /
  ui 34 fixture / doctest 50 全绿；Cursor 使用面收敛到 parse 层。

### where 部分整理（6 阶段链路核对 + 3 处小修）

- 链路：where_process（裸 where 重写）→ parse_primitive（尾部剥离
  TyWithWhere）→ apply（组合）→ extract_impl_parts（where_clauses 提取）→
  trait_bounds（trait where 合并）→ resolve_where_at（@N 解析）；
- `where` 末尾缺 body 提前报错（去 `i + 1 < len` 短路——where 是 Rust 关键字，
  Ident `where` 只可能是 DSL 形式）；`tokens[i+1]` 越界 → `get`；
  scan_body_boundary 的 `Vec<&TokenTree>` + cloned → 直接收集；
- 验证：全绿（同上）。

### parse 层打扫

- parse/mod.rs 354 → 339：`cursor.peek().map(...).unwrap_or_else(call_site)` ×3
  收敛为 `cursor_span`；WithAttr/WithPrefix 半应用分支（rest 空 vs apply）
  收敛为 `attach_wrapper`（TyKind + rest + trait_name）；
- parse_atom.rs：parse_range 的 `TyKind::Range(TyRange {...})` → `TyRange {...}.into()`
  （统一 into 风格）；
- 验证：全绿；parse 层全部 ≤350（339/199/128）。

### ast 层拆分（types.rs 470 → 4 文件全 ≤350）

- `types.rs` 470 → **261**：子类型定义 + Ty/TyKind + Op + MAX_EXPAND +
  count_leaves + fresh 系列；
- 新 `types_visit.rs`（159）：Expand 枚举 + expand_wrapped/expand_rebuild +
  `Ty::map_children` + `Ty::expand`（遍历归一处）；
- 新 `types_from.rs`（77）：`impl_from_for_ty!` 宏（子类型 → TyKind/Ty/
  Box<Ty> + to_ty）+ 19 变体调用列表（含 TyError）；
- types_render.rs（169）保持；ast/mod.rs 聚合 re-export；
- 验证：全绿。

### codegen 层打扫

- generate_impl：删 `let parts = parts;` 影子绑定（NLL 已覆盖，历史遗留）；
  `Ty::new(call_site, TyPrimitive(...).into())` → `TyPrimitive(...).to_ty()`；
  HashSet 显式导入；resolve_where_at 过时注释修正（blanket wrapper where
  的 @N 现在也走这里——@0 统一后的文档同步）；
- impl_parts.rs：extract_impl_parts 的 4 处双构造（WithCode/WithWhere/WithAttr
  None 分支 + WithPrefix 目标包装）统一 to_ty 链式；测试 1 处同步；
- ast/types_visit.rs：map_children 的 `redundant_closure` allow 属性在 ast
  拆分时丢失——加回（`&mut FnMut` 不能被 move 进 `.map(f)`）；
- 验证：全绿；codegen 层 311/145 行。

### entry 层打扫

- expand_attr_macro：path prefix 分支收敛——`if !include_trait { match } else
  { 重复 None 分支 }` 改为 `(!include_trait).then(|| try_parse_path_prefix(...))
  .flatten()` + 单一 match（None 分支只写一次）；
- 新增 `Cursor::span()`（当前位置 span，at_end 时 call_site）——entry 3 处
  `peek().map(...).unwrap_or_else(...)` 收敛；trait_path 的 first() 处
  `map_or_else` 同步；`path_prefix::` 模块路径改 use；
- 验证：全绿；entry 层 286/67/68。

### 文档规则（作者措辞 + 不标开发中）

- 文档中**项目所有者/设计意图**用"作者"；**库使用者**保留"用户"（如
  "用户自定义常量""用户泛型""用户可见"——使用 batch-impl 写 DSL 的人）；
  代码标识符（user_table 等）与源码错误消息不改；
- 版本状态标记不写"（开发中）"（避免遗漏）；中文 doc 头部直接写版本号。
### util 层打扫

- `scan_with` / `scan_stop` 双名合并：scan_with 无外部使用者（全走
  scan_stop 转发）——scan_stop 内联实现，删转发层；
- scan_stop 的 `->` 箭头 guard 用 `is_joint_punct_at`（统一 punct 工具）；
- `Cursor::is_punct` 委托 `is_punct_at`（消除重复 matches!）；
- 验证：全绿；util 层 161/41/11。

## 0.6.4 (2026-08-05)

### `@trait` 提前展开（常量阶段/段级），`@N` 成为唯一 codegen 记号

- 作者指出：`@trait` 不该留到 codegen（只有 `@N` 需要 impl 泛型列表）。
  结构性原因：`where{...}` 是 Brace 组，`expand_consts` 不进入（body 的 `@`
  是 pattern 语法）——where 谓词里的 `@trait`/`@N` 都残留到
  `resolve_where_at`；
- 三处修复：
  - `expand_consts` 识别 `where` Ident + Brace 组（DSL 结构非 body）→ 进入
    展开 `@trait`（batch_impl 用 trait 路径）；`@N`（`@` + Literal）在
    `try_expand_at` 返回 None 保留（不再误报"must be followed by a name"）；
  - `replace_segment_trait`（batch_trait! 段级）递归进组——where 谓词里的
    `@trait` 也能段级替换；
  - `resolve_where_at` 删 `@trait` 分支（trait_name 参数移除）——只剩 `@N`；
- 验证：batch_impl `where{T: @trait<T>}`、batch_trait! 段级 where 组内
  `@trait`（探针）都提前展开；纯 fresh `where{@0: Clone}` 回归全绿。

### Apply trait 恢复：`apply` 右分发默认实现（span 兼容）

- span 改造时 `trait Apply` 只剩 `apply_help`（右分发被挪到 `TyKind::apply`
  普通方法）——trait 名与主方法名不一致；恢复之前设计：
  - `trait Apply: Clone + Into<TyKind>`——`apply(self, o, span)` 默认实现
    （右操作数结构分发，从 `TyKind::apply` 平移）+ `apply_help` 抽象钩子；
  - `impl Apply for TyKind`（覆写 `is_type_param` + 转发子类型）；
    子类型 `apply_help` 改普通方法（`impl X`，`pub(crate)`）——不再实现
    trait（默认 apply 的 `Ty::new(span, self)` 需要 Self: Into<TyKind>，
    子类型不满足——编译期验证）；
  - `is_type_param()` 默认方法（TyKind 覆写）替代 `matches!(self, ...)`——
    泛型 Self 无法 match TyKind 变体（E0308 抓出）；
- span 贯穿不变：`Ty::apply` 取 span → `kind.apply(o, span)`（trait 默认，
  每个构造 `Ty::new(span, ...)` 用左操作数 span，`o.span` 仅 fallthrough）；
- 测试全绿（分离声明顺序、数组/范围/泛型外提均回归）。

### `@N` 语义修正（作者设计评审）

- 作者初衷：`@N` 应是 `_Param_N_BatchGen_` 的直接映射（宏元层常量）——但 fresh
  编号是全局计数器、与最终 impl 泛型位置无关（多 fresh 源/用户泛型混排时错位），
  直接映射不可靠；
- 定案：`@N` = where 谓词内**第 N 个 fresh 泛型**（`_Param_{N}_BatchGen_` 形式）。
  `resolve_where_at` 把 impl 泛型列表过滤出 fresh 形式后按位置取——用户泛型
  直接写名字；blanket 包装谓词 `@0`（= 唯一 fresh T）与新规则自然统一，不再
  是特例；
- 破坏点：B1 测试 `where{@0: @trait<T>}` → `where{T: @trait<T>}`；
  tutorial AtWhere 示例同理；越界报错文案更新；
- 测试：`()^2 where{@0: Clone, @1: Copy}`、`()^3 where{@2: Clone}`（纯 fresh）
  不变全绿。

### 泛型参数族 + 分离声明顺序修复

- 新增 `@all_type_params` / `@all_const_params` / `@all_lifetimes`：
  `GenericFilter` 枚举 + `resolve_generic_marker` + `get_trait_generic_decl`
  （helpers.rs），展开为**扁平** `<...>` 声明（angle_collect 统一配对）；
  类型参数只名字（bound 走同名继承）、const 完整（裸名 E0747）、生命周期原样；
  try_expand_at 在 @all 分支后分发（batch_impl-only，batch_trait! 报错）；
  无该类参数时报错；
- **顺带修复真实 bug**：`TyKind::apply` 的 WithType hoist 分支（`T^<A>X` →
  `<A>(T^X)`）对"声明 apply 声明"（`<'a> <T> X` 连续声明）错误地把内层参数
  提到外层 → 生成 `<T, 'a>`（lifetime must be prior）。修复：self 是
  `TyKind::TypeParam` 时走 `apply_help` 保持声明顺序（`<'a, T>` lifetimes
  first）。手写 `<'a> <T>` 此前也炸——测试 `generic_param_families` 锁定
  组合形态；
- 测试：dsl 51（type/lifetime/const 三族 + 组合 + bound 继承）；ui
  `generic_family_batch_trait`（batch_trait! 报错）。

### 常量名字族改名（作者拍板）

- 提案：`@i*`/`@u*`/`@f*` 取代 `@uint`/`@int`/`@float`（族符号统一——原
  `uint` 与范围族 `u8` 的 `u` 不一致）；`@u8..64` 宽度缩写提案被否（收益小、
  引入"族从左端点继承"的隐藏规则）。
- 实现：`builtin_named` 的 `"u*"`/`"i*"`/`"f*"` 通配（try_expand_at 检测
  `tokens[2]` 为 `*`，lookup = `name*`，consumed 3）；`check_value_refs` 同步
  通配识别（`@uints=@u*` 值内引用曾误报 "unknown @u"——修后懒展开链完整）。
  错误消息 builtins 列表与 `@` 后缺名示例更新；ui `const_unknown` 快照重生成。
- 测试：dsl `@uints=@u*`（batch_trait 值内通配引用）、`[Box, Rc]^@u*`（宏变量
  None 组内通配）全部更新通过；直接 `@u*` 探针验证 usize 含入。
## 0.6.3 (2026-08-05)

### 文档修正

- 作者指出 README 头部示例错误：`#[batch_impl(()^4)]` 的 `// →` 注释声称展开为
  4 个不同长度的元组 impl（`(A,)` 到 `(A, B, C, D)`）——实际 `()^N` 是**单个**
  N 元组（`()^4` → `(A, B, C, D)`），多长度是 `()^1..=4` 范围语法（教程 §11 表
  一直正确，测试 `tuple_pow_basic` 锁定语义）。实测探针确认后修正 README 中英
  两版的注释（`impl<A, B, C, D> TupleTrait for (A, B, C, D) {}`）。仅注释修正，
  无行为变化。
- Cargo.toml bump 0.6.2 → 0.6.3（0.6.2 已发布）。
## 0.6.2 (2026-08-05)

### 基于 span 的诊断（L3）

- **结构改造**：`enum Ty` → `struct Ty { span: Span, kind: TyKind }`（变体级 span
  被否决——"span 放 Ty 层，不放 TyNum"）；`TyKind` 以普通方法承载右操作数分发
  （`TyKind::apply` / `TyKind::apply_help`），`trait Apply` 只留
  `apply_help(self, o, span)`（bound 为 `Clone + Into<Ty>`——TyKind 无法满足
  `Into<Ty>`，故用普通方法而非 trait）；`Ty::apply` 取 span 后委托——span 贯穿
  的唯一入口；
- **递归修复**：迁移时 `TyGroup::apply_help` 被改成"包回 Group 再 apply"，
  `o` 为普通类型时无限递归（fuzz `parse_no_panic` / `full_pipeline_no_panic`
  栈溢出）；改回 `self.0.apply(o)`（组的透明性）。fuzz 抓到了它——no-panic
  承诺的价值所在；
- **诊断层**：`compile_error_str(msg, span)`；ident-span 方案——
  `Ident::new("compile_error", span)` + `quote!`（括号/字符串/分号保持
  call-site），因为 `quote_spanned!(span => compile_error!(...))` 会让 rustc
  把错误当作 item 位置的用户代码（"macros that expand to items must be
  delimited with braces..."）；新增 `compile_err_at!(span, ...)` 宏；
- **接线**：parse（cursor/op 的 span——`^` 缺操作数现在指向 `^`）、consts
  （`@` 引用 span）、blanket 包装、where_process、entry、lib、codegen；
  apply 错误用 `err_ty_at`（span 参数已由 `apply_help` 贯穿）；
- **平台限制（已记录）**：属性宏输入 span——顶层 token 精确、组内 token
  退化 call-site、`Err` 返回的错误显示在宏调用行。精确 span 只出现在
  Ok 输出的 `Ty::Error` 路径（parse/apply）。这是 rustc 行为，宏侧无法修复；
- ui 快照经 TRYBUILD=overwrite 重新生成（span 变化移动了错误位置）。

### 按 receiver 种类的 `@all` 过滤（L1）

- `ReceiverFilter` 枚举（Ref / Value / Static）+ `AllMarkerSpec` 类型别名在
  `helpers.rs`；`resolve_all_marker` 表新增 `all_ref_methods` /
  `all_value_methods` / `all_static_methods`，`get_trait_item_names` 增加
  receiver 过滤维度；
- syn 3 receiver API：`f.sig.receiver()` 返回 `Option<&Receiver>`，其
  `kind: ReceiverKind` 为 `Value` / `Reference(..)` / `Typed(..)`
  （syn 2 风格的 `receiver.reference` 字段已不存在——E0609 抓出，
  改为匹配 `ReceiverKind`）；
- 动机：blanket 的 by-value 委托语义模糊（展开时无法判定 Deref/移动能力）；
  `#blanket(@all_ref_methods)` 让用户只委托 `&self`/`&mut self` 方法、
  by-value 方法保留 trait 默认实现；
- 测试：`receiver_kind_filters`（ref/mut/val/static 各被正确标记选中）+
  `blanket_receiver_filter`（Box blanket 委托 `by_ref`、`by_val` 回落默认——
  注意默认实现需要 `where Self: Sized`，因为默认方法里的 `self` receiver
  要求它，E0277）；
- 文档（zh-CN）：tutorial 常量表 + architecture 的 `@all` 描述与指令表已更新；
  英文镜像发布时补。

### `#blanket` 静态方法委托（F1，重构）

- 评测员报告：`#blanket(@all_static_methods)` 生成 `(**self).make()` —
  E0424（关联函数没有 `self`）。blanket 的既有漏洞（委托体总引用 self），
  被 L1 静态过滤暴露；
- 第一版修复：守卫 + 指向 `#fill(@all_static_methods)` 的报错（评测员方案 A）；
- 设计评审后重构：委托严格更优——静态方法没有可 deref 的 receiver，但
  blanket impl 携带 `t: Trait`，`t::make(...)` 与 `<t as Trait>::Item`
  投影完全同构。`expand_blanket` 现在按 receiver 选择委托体：
  `(#self_ty).#name(...)`（有 receiver）vs `#t::#name(...)`（无 receiver）。
  dsl 测试 `blanket_static_delegation` 锁定直接、链式（`Box<Box<u8>>`）与
  参数转发三种形态；临时 ui 报错 fixture 已删除。符合 blanket 哲学：
  实例方法经 deref 转发、静态方法经 bound 转发——都是转发，不特判。

### 全英文化（注释、错误消息、文档）

- **范围**：`src/` 全部中文注释（`//`、`///`、`//!`，29 文件 ~356 处）与
  `tests/`（28 .rs + 31 .stderr）译为英文；59 条 `compile_err!` /
  `compile_error_str!` 消息全部翻译；消息中的 DSL 记号原样保留；
- **过程**：5 个并行子代理按模块分组（preprocess / parse+apply /
  ast+codegen / entry+util+analyze+testing+lib / tests），每组带
  "绝不改动代码逻辑"的硬规则；ui `.stderr` 快照经 `TRYBUILD=overwrite`
  重新生成（56 文件）——权威消息文本以实际输出为准，快照从真实输出重写；
- **翻译后清理**：子代理嵌套列表引入的 clippy `doc list item without
  indentation` 警告，把 doc 注释拍平为散文修复；
- **文档**：中文文档移入 `docs/zh-CN/`（冻结归档），英文版原地书写
  （README / CHANGELOG 全量 19 版本条目翻译 / tutorial 816 行 40 个 rust
  块逐字保留 / architecture / dev-changelog）；二次扫描翻译了 doc 代码块
  **内部**的中文注释（仅 rust 块的 `//` 注释，代码 token 不动）；
- **损坏围栏修复**：tutorial 段级 `@trait` 示例的围栏损坏
  （`` `ust `` — backtick + CR + `ust`），修成 ```rust 后作为 doctest
  编译通过；块内容与通过的 `tests/dsl.rs` 段级测试一致，安全；
- **验证**：fmt 干净、clippy 零警告、`cargo test --all-targets` 全绿
  （lib 10 / dsl 46 / regression 26 / ui 全部 fixture）、doctest 46
  （原 45，+1 修复块）、`src/`、`tests/` 与全部英文 doc 中文零残留。
## 0.6.1 (2026-08-05)

### 递归深度护栏恢复（0.1 承诺的回归修复）

- 复盘发现 0.1.0 的「递归深度限制（128 层）」在 0.3.0 重写时丢失：实测 30000 层
  `[[[...]]]` 与 `Vec<Vec<...>>` 嵌套导致 `STATUS_STACK_OVERFLOW`（abort 非 panic，
  fuzz 深度 3 测不出）；
- 恢复：`angle_collect` 拆出 `angle_collect_at(tokens, depth)`，4 处递归点
  （None 组扁平化 / Paren / Bracket / `<>` 内容）depth+1，`MAX_NEST_DEPTH = 128`
  超限报「嵌套深度超过 128 层」——入口拦截后下游 consts/expand_tokens/parse/
  codegen 的组深度全部 ≤ 128；
- 附带：`parse_primitive` 连续 body/where 附着（`T{a}{b}`）从递归改**迭代**
  （attaches 栈收集 + 从内到外 apply）——线性链本不该递归，消除该递归源；
- 边界澄清：>128 层被宏内拦截；**数万层 `[` 嵌套的崩溃发生在 rustc tokenize
  阶段**（宏被调用前，任何 proc-macro 库无法拦截的外部边界）——128 层远低于
  rustc 阈值，合法输入永不触发；
- 测试：ui fixture `deep_nesting.rs`（200 层 `[`）+ angle 单测
  `angle_nesting_limit`（129 层组）。

### 文档修正：`batch_trait!` 指令缺口如实声明（不改代码）

- 实测确认 `expand_tokens` 仅 `expand_attr_macro` 调用——`batch_trait!` 从未做
  指令展开，`#fill` 等直接报 `found '#'`；而 lib.rs:111 / tutorial.md 原声称
  "与 `#[batch_impl]` 相同语法"（虚假承诺）；
- 决策：**不改代码**——`batch_trait!` 保持函数式宏纯 spec 语义（加入 trait
  定义是 `#[batch_impl]`/`#[batch_impl_only]` 的职责）；run_pipeline 的
  `start_trait`/`trait_bounds` 参数已预留，未来若扩展语法可直接接入；
- 修正 lib.rs `batch_trait!` doc + tutorial.md 对应章节：`:` 右侧为类型 DSL +
  `@` 常量，`#` 指令需属性宏入口；CHANGELOG 0.6.1 条目同步。
- 与 0.5.6（`A<>` 透传）/ 0.5.7（bound 不继承）限制同源：指令域依赖 trait
  定义，仅属性宏入口可用。

### 模块重组：文件夹 mod + 文件（消除"平"结构）

- 根下 10 个平文件收编为分层目录，每目录 `mod.rs` 聚合 re-export
  （引用侧统一写目录级 `crate::xxx::X`，不写子模块路径）：
  - `entry/`：入口与驱动（原 `expand.rs` → `mod.rs`、`batch_trait_entry.rs` →
    `driver.rs`、`path_prefix.rs` 收编）；
  - `preprocess/`：token 重写器（原根下 `consts.rs`、`empty_generics.rs` 移入，
    `preprocess_helpers.rs` 更名 `helpers.rs`）；
  - `analyze/`：trait 定义语义分析（原 `trait_bounds.rs` 移入）；
  - `util/`：共享工具（原 `scan.rs` / `diagnostic.rs` 移入，mod.rs 聚合）；
  - `testing/`：测试基建（原 `fuzz.rs` 移入，`cfg(test)`）。
- `parse/` `apply/` `ast/` `codegen/` 四层不动；lib.rs 仅剩宏声明 + 模块树。
- 依赖方向单向：util → ast → parse/apply → preprocess/analyze → codegen →
  entry → lib。

### 逻辑合并（D 阶段，去重而非删注释）

- `trait_bounds::generic_param_names`：blanket.rs / empty_generics.rs 的泛型
  参数名收集循环收敛为共享函数；
- `parse::parse_binary_chain`：`-`（左结合）与 `^`（右结合）两分支骨架同构，
  收敛为参数化函数（错误消息保留 `（如 T-U）` 示例后缀，ui 快照不变）；
- `types_render::render_param` / `render_optional`：codegen impl 泛型渲染复用
  单条声明渲染；WithPrefix/WithAttr/WithCode/WithWhere 四臂双态渲染收敛；
- `apply_tuple` 两宏：WithTrait/WithType/WithCode/WithWhere 四类包装的
  "透传到内层再重包" apply_help 宏化（教训：`self.1` 作宏参数会因调用处
  hygiene 解析为模块 self——E0424，字段访问必须写在宏体内）；
- `fuzz::full_pipeline_no_panic` 改走真实入口 `expand_attr_macro`（此前手写
  管线漏掉常量展开与 `A<>` 照抄，fuzz 覆盖路径与线上不一致）；
  `expand_attr_macro` 改收 proc_macro2 类型使单元测试可调，lib.rs 入口转换；
- 放弃三项（有理由）：路径收集统一（path_prefix 严格状态机 vs 段循环宽松
  收集，统一会劣化诊断）、expand_wrapped/expand_rebuild 合一（需引入
  expect 违反"永不 panic"）、consts 换 scan_stop（无重复可换）。

### 评测修复（评测员 B1-B4 + 补充测试）

- **B1（真 bug，一行）**：codegen/mod.rs 的 @trait 分支写 id == "Trait"
  （大写）——普通 where 谓词（where{@0: @trait<T>}）的 @trait 被错误拒绝、
  错误消息自相矛盾；全库其余 4 处均小写。**教训**：dev-changelog 此前声称
  "resolve_where_at 同步小写"实际未替换——PowerShell Select-String 大小写
  不敏感的反噬（残留检查误报通过）。测试：dsl `review_fixes_locked`
  （B1 场景 + 自引用 bound 需补 impl WhereAtTrait<u32> for u32）。
- **B2（回归隐患）**：新顺序（@ 先于 <> 配对）下 expand_consts 运行时
  真实 None 组（宏变量 $(...)*/$x:ty 展开产物）当时未被 angle_collect
  扁平化——组内 @ 不再展开（0.6.0 顺序可以）；原注释"真实 None 组已由
  angle_collect 在入口扁平化，此处永远不会出现"在新顺序下不成立。修复：
  expand_consts 加 delimiter![none] 分支——新顺序下 <> 组尚未存在，
  None 组必是真实透明组，无旧歧义（0.6.0 曾踩过的 delimiter![none] 误伤
  尖括号组问题不复存在）。测试：dsl `review_fixes_locked`（宏变量 + 组内
  @uint 探针实测，2024 edition 下 gen 是保留字、宏名须换）。
- **B3（文档）**：@all_default_types 依赖 trait 关联类型默认值
  （`type T = u8;`）——nightly（`associated_type_defaults`，stable 报
  E0658）——tutorial 标注该标记仅 nightly 场景可用（@all_required_types
  的 `type T;` 声明 stable 可用）。
- **B4（防御）**：`batch_trait!` 定义 @trait=[...] 常量会被特殊记号拦截、
  被段级替换静默遮蔽——collect_user_consts 拒绝 `trait` 作常量名
  （"保留记号"报错）。
- 评测员补充测试 dsl 35 节 macro_meta_review_extras（正向路径全覆盖：
  @all_required 全种类 / @all_default_constants / 标记减法 /
  @trait<T> 顶层 spec / [a,b] 于 #delegate / blanket where 仅 @0 /
  ()^3 where{@2: Clone} 多参数位置引用）——全部通过。

### 宏元层完整化（0.6.1 主线：`@` 唯一宏元记号 + blanket 约束合并）

- 背景：作者提出「`#all` 看着不顺眼，违反 `#` 的两种格式」——`#` 应
  只剩指令名；范围选择（选哪些 item）是宏元层操作，统一归 `@`；
- `@all` 系：try_expand_at 加分支（`resolve_all_marker` 抽公共表——指令域
  与宏元层共用），展开为 Bracket 组（`render_list_strings`）；batch_impl
  专属（需 trait_def），batch_trait! 报错；`#all` 系全删（parse_marker 删除、
  parse_name_tokens/parse_minus_target 的 `#` 分支删除）；
- 指令参数支持 `[a,b]`（递归解析组内容；空组报错；`-` 排除支持
  `-[a,b]`）——`@all` 展开产物即此形态，用户手写等价；
- trait 感知常量（ConstCtx::Attribute 携带 trait_def）：`@trait` 展开本地
  trait 名；`@Cow` 内置（`Cow<'_>` + 固有约束谓词——quote 不配对尖括号，
  ty 须手动 `Group::new(delimiter![<>])`；与砍掉的裸类型名常量不同类：
  携带约束才有复用价值）；
- blanket 包装约束谓词：尾随 `where{...}`（在 `:N` 后）并入 impl where；
  `resolve_target_predicates` 处理 `@0`（→ fresh T）与 `@trait`；
  **教训**：`quote!(where { #(#wrapper_preds),* })` 会把每个 TokenTree 当
  列表元素逗号连接——谓词流须整体插入；
- `<>` 只留名字：blanket 泛型声明 TypeParam 只取 ident、const/lifetime
  原样（纯名字 `N` 会 E0747）；`T: Trait` 进 where 基础谓词（与包装谓词
  并列合并）；trait 形参 inline bound 由 codegen 继承逻辑处理（曾转移导致
  X: Clone 重复——继承按位置补 bound，见 `gen_where_probe` 实测）；
- `@0` 通用化：codegen 渲染 where 谓词时替换 `@N`（→ impl 泛型第 N 位名字）与
  `@trait`（→ trait 名）——元组 `()^2 where{@0: Clone}` 与普通 spec
  `where{@0: Default}` 可用（此前仅 blanket 包装 where 特化：`@0` 恒指
  目标泛型 fresh T，由 resolve_target_predicates 预替换，两处不冲突）；
  越界/格式错误并入 errs 收集报错（generate_impl 非 Result 返回）；
  测试 dsl `where_position_refs`。

- `@Trait` → `@trait` 改名 + 路径化：内置名族全小写统一（`@uint`/`@scalar`/…）；
  内容从「本地 trait 名」改为「trait 完整路径」——`batch_impl` = 本地名、
  `batch_impl_only` = 外部路径（`#ext::Trait:` 前缀）——blanket 包装 where
  写 `@0::Owned: @trait` 免手写路径；实现：路径前缀解析**提前**到 `@` 展开前
  （`@trait` 需要 trait_full_path；ConstCtx::Attribute 加 trait_full_path 字段、
  trait_full_path() 访问器）；blanket 的 resolve_target_predicates 改用
  trait_full_path（原 trait_def.ident 只给本地名，外部场景错）；codegen 的
  resolve_where_at 同步小写；**教训**：PowerShell Select-String 大小写不敏感，
  残留检查误报（实际已替换）。

- `batch_trait!` 段级 `@trait`：多段每段 trait 名不同，常量值（如
  `@type_t=<T>@trait<T>`）里的 `@trait` 由 entry 分段循环逐段替换为本段
  trait 路径（`replace_segment_trait`）——跨段复用「泛型声明+trait 名」打包；
  实现要点：try_expand_at 改返回 `Option`——Trait ctx 的 `@trait` 返回
  `None`（原样保留、不触发懒展开递归——展开为原样→再遇→栈溢出的死循环，
  实测 STATUS_STACK_OVERFLOW）；check_value_refs 跳过 `@trait`（特殊记号
  非常量引用）；测试 dsl `trait_const_segment`（教训：trait 定义须带泛型
  匹配 spec 的 `<T> Trait<T>`；`Box^[T,(T,)]` 泛型重叠 E0119 是用户写法
  问题，测试改用 `[T, Vec<T>]`）。

- 测试：dsl `macro_meta_complete`（@trait/@Cow/blanket where/[a,b]/where
  规范）、`trait_const_value_with_angles` 保持；全量回归绿。

### 预处理顺序修正：`@ <> # where`

- 背景：作者提议宏元层（`@`）应是最外一趟。实测当前顺序（`<> @ #`）
  的 bug：`batch_trait!( @inner = Vec<u8>; @outer = Vec<@inner>; ... )`
  ——`Vec<@inner>` 的 `@inner` 被 angle_collect 配对进尖括号组，而
  expand_consts 刻意不进入 `<>` 组（`delimiter![<>]` 与真实 None 组
  展开值相同不可同臂，注释已记录）——`@` 残留报 `found '@'`；
  直接值 `@map = HashMap<u32, String>` 恰好因定义处配对兜底不炸，
  嵌套/引用场景暴露；
- 修正：entry 两入口把 `collect_user_consts` + `expand_consts` 移到
  `angle_collect` 之前——`@` 展开产物（可能含扁平 `<...>`）统一由
  后续 angle_collect 配对；`#` 指令与裸 where 改写位置不变；
- 能力矩阵：batch_impl/only = 内置 `@` + `<>` + `#` + where；
  batch_trait! = 自定义 `@` + `<>` + where；
- 测试：dsl `trait_const_value_with_angles`（`@map` 直接值 + `@outer`
  嵌套值；E0252 教训——dsl.rs 已 use HashMap；E0119 教训——batch_trait!
  自身生成 impl，勿手写重复）。

### 新范围标记：`@all_required*` / `@all_default*`

- 背景：`@all` 系一直未区分 trait item 的默认实现状态（`#fill(@all)` 连有
  默认实现的也覆盖，`@all + -name` 逐个排除繁琐）；作者提出按状态过滤；
- 实现：`get_trait_item_names` 加 `default: Option<bool>` 过滤参数
  （`Some(true)` 仅默认、`Some(false)` 仅 required、`None` 全含），
  syn 判断字段：`TraitItem::Fn(f).default` / `Const(c).default` /
  `Type(t).default`（fn=默认体、const=默认值、type=默认类型）；
- `parse_marker` 改表格分发（kinds, default）——12 个标记内联，删除
  `get_all_trait_methods/items/constants/types` 四个薄 wrapper；
- 语义要点：`@all_required*` 单独用完整（只填必须的、默认保留）；`@all_default*`
  单独用缺 required → E0046，须与 required 侧/手写组合；required ∪ default = all；
- 测试：dsl `all_default_required_markers`（fill 组合 / fill 只 required /
  blanket 只 required 三场景；E0034 教训：三个 trait 须各占一个整数类型）；
- 三指令（fill/delegate/blanket）共享 `parse_names_from_tokens`，一处改全部获得。

### 旧测试用例抽查（git 历史）——发现并修复 `T^<A,B>` 参数丢失

- 对照 v0.5.0 删除的 examples/{tests,ds_tests,my_tests,debug_tests}.rs
  （~4800 行）与当前 dsl/regression 测试矩阵，4 个候选盲区实测：
  - `[&, self]^[u32, i64]`（前缀混合列表叉积）、
    `()-[usize, isize]-[u32, i32]`（空元组双列表减法链）——行为正确，已覆盖；
  - `HashMap^<u32, String>`（caret 后跟泛型参数列表）——**真 bug**；
  - `[usize #fill(@all){..}, isize #fill(@all){..}]`（列表元素独立指令）——
    与 dsl `directive_fill` 重叠，未单独补。
- **bug 根因**：parse_primary 顺序缺陷——单个 `Group(<>)` 输入在
  `[TokenTree::Group] → parse_group` 分支被抢先拦截，parse_group 不认
  `<>` 组落 `_ => empty()`，而 `parse_type_params`（本应处理 `<A,B>` 独立
  操作数）永远到不了；带 body 时 empty 被 `TyWithCode` 包裹后逃过
  `is_empty_operand` 检查 → `<u32, String>` 静默丢失、输出裸 `HashMap`，
  无任何诊断（不带 body 则报"`^` 后缺少操作数"，行为分裂）；
- **修复**：`[Group] → parse_group` 分支排除 `delimiter![<>]`，尖括号组
  落到 parse_type_params——按 apply/mod.rs 注释既定语义
  `T^<A,B> => T<A,B>`（`HashMap^<u32, String>` → `HashMap<u32, String>`）；
- 测试：regression `caret_angle_param_list`（`contains_key` 断言 impl 落在
  泛型完整类型上，防退化为裸 `HashMap`）。

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
- CHANGELOG 拆分为作者版（CHANGELOG.md）与开发者版（本文件），0.1.0 →
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

## 项目演进史

> 每代一句话主线（正式版本）：
> **0.1 发布** · **0.2 属性与前缀**（fn/指针/`#[attr]`/assoc）·
> **0.3 重写**（手动重建统一模型）· **0.4 指令系统**（`#fill`/`#delegate`/开放扩展）·
> **0.5 where 系统**（`where{...}` + bound 继承 + `A<>` 照抄）· **0.6 常量系统**（`@` 名字族/范围族/自定义）。
> 0.1.0 之前的两代原型（crate 原名 `auto_impl`）与 0.2 的重写动机，见下。

### 早期结构对照（crate 原名 auto_impl 起，至 0.2 重写前）

### 0.-1 (2026-07 原型，684 行单文件)

- **静态类型列表**：spec 是"泛型 + trait泛型 + 目标 + body"的顺序结构，
  无 `^`/`-` 运算符、无元组生成、无前缀系统——目标类型是 token 透传的静态类型
- 但 **80% 的设计已定稿**：`[]` 歧义（逗号=列表/无=切片）、`()` 分组 vs 元组、
  泛型继承（子项追加父级）、body 继承（列表级共享/子项覆盖）、
  trait 泛型悬空诊断（"`MyTrait<T>` 被解析为 trait 泛型参数，但缺少目标类型"）、
  `compile_error_at` span 定位、中文错误消息
- **trait 泛型自动补全**：trait 有泛型时从 `trait_generics` 自动补全
  （`#trait_name<#(#params),*>`）——0.0 因 `^` 引入砍掉，0.5.5 的 `A<>` 照抄回归

### 0.0 (2026-07 原型，1961 行单文件)

- **灵光一跃：类型组合运算符化**——`^`（右结合：`A^B=A<B>`、`&^T=&T`、
  `[A]^[B]` 笛卡尔积）、`-`（左结合元组构建）、`()^N`/`^M..N` 元组生成、
  fresh 泛型（`A_7f3a_` span 位置哈希后缀）、前缀系统（`&`/`&mut`/`self`/`unsafe`）、
  递归护栏（`RecursionGuard` 128 层，第一天就有）——DSL 的全部核心概念在此定稿，
  之后 0.1→0.6 未再引入新概念，只有精化与外围系统
- 已埋的缺陷（0.2.1/0.2.2 才修）：`split_raw` 无 `->` 守卫、
  `expand_caret` 右侧无 dash 分割（`HashMap^K-V` 解析成嵌套而非并列）

### 0.1.x (2026-07 首个发布系列)

- **模块拆分完成**：0.0 单文件分节直接切为 `core/` 9 文件
  （types/recursion/utils/codegen/tuple/caret/dash/parser + lib.rs 入口）——
  0.2 的 9 文件结构就是它；
- **prefill 预填泛型**（`HashMap<K>^V → HashMap<K, V>`）：`PrefixItem::Container`
  加 `prefill` 字段，caret 与 dash 两条路径都接入；
- 递归护栏原样保留（`RecursionGuard` 与 0.0 逐字相同）；
- 0.1.1 尚无：fn/指针/属性前缀（PrefixItem 仅 6 变体）、assoc 绑定
  （ImplSpec 5 字段）、全局 `->` 守卫（仅 dash 局部有）——0.2.0/0.2.2 补。

### 0.2 (2026-07-19，9 文件 3197 行)

- 在 0.1.1 结构上延续：+`fn`/`*const`/`*mut`/`#[attr]` 前缀变体、
  +assoc_bindings/attributes 字段、+`->` 全局守卫（0.2.2 统一）；
- BUG-1/2/3 集中爆发（`^` 右侧 dash 分割、`parse_balanced` pos 丢弃、
  前缀链过滤）——"按操作符组织 + 深度散落"模型走到极限，0.3.0 重写。

> **重写动机（作者注）**：0.2 之前是"阐述设计思路 + AI 增量实现"——
> 思路一个个蹦出，架构随补丁生长，无人完整持有整体模型；0.2.x 时修改一个
> 常识级 bug（如 `->` 守卫）要定位半天——深度逻辑散落五处、`^`/`-` 双实现，
> 改一处须确认其余各处行为一致。于是 0.3.0 由作者**手动重写**：先重建统一
> 模型（优先级链 + Apply trait + Ty 枚举），安全设施（递归护栏）未随模型
> 重建，直到 0.6.1 回归（见 0.6.1 段）。
> 0.3 之后架构稳定的真正原因不是重写本身，而是**模型从此由作者完整持有**——
> 每一行都知道为什么，改 bug 不再需要跨散落处核对。

### 三条"砍掉又回归"暗线

- **trait 泛型自动补全**：0.-1 有 → 0.0 砍（`^` 引入后 trait 名后 `<...>` 歧义）
  → 0.5.5 `A<>` 照抄回归；
- **递归护栏**：0.0 有 → 0.3 重写从零开始时丢失（未重建）→ 0.6.1 恢复
  （`MAX_NEST_DEPTH`，见 0.6.1 段）；
- **body 合并语义**：0.-1/0.0/0.1.1 子项覆盖列表级 → 0.2 改拼接
  （独立 body 与共享 body 合并，同名方法由编译器报错）。

### 行数演进

`684 (0.-1) → 1961 (0.0) → ≈2153 (0.1.1) → 3197 (0.2) → 1628 (0.3.0 初版)`
`→ ≈1586 (0.3.0 正式版，五文件) → 4400 (0.6)`


