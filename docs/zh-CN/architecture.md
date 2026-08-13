# batch-impl 内部架构

**v0.7.2**——0.7.1 已发布：定向诊断 + 单一真相源笛卡尔积（`util::cartesian`）+ 指令分发迁入 `directives/`；0.7.0：**splat** `*` 前缀（`TySplat{Tuple,Array}` 枚举镜像来源括号，完整委托 `TyTuple`/`TyArray` apply + 包回）、数组分发传播、parse 层拆分 `chain`/`primary`/`trailing`；0.6.x：预处理顺序 `@ <> # where`、宏元层完整化、`@N` fresh 引用、receiver 过滤、blanket 委托、span 诊断。

面向贡献者：模块组织、解析流程、错误机制、测试矩阵。

## 模块组织

```text
lib.rs              宏入口（#[batch_impl] / #[batch_impl_only] / batch_trait! / 测试宏）+ 模块树
  ├── entry/                入口与驱动
  │   ├── mod.rs            入口实现：expand_attr_macro / expand_batch_trait + 公共管线 run_pipeline
  │   ├── driver.rs         共享驱动：BFS 展开并列列表 → 逐叶子 generate_impl
  │   ├── preview.rs        batch_preview!：诊断通道展开预览 + `^`/`-` 误写提示
  │   └── path_prefix.rs    外部 trait 路径前缀：#Path::to::Trait: 状态机解析
  ├── analyze/              trait 定义语义分析
  │   └── trait_bounds.rs   TraitBounds / TraitParam + syn AST 引用收集（where 谓词透传槽位）
  ├── util/                 共享工具（mod.rs 聚合 re-export，引用侧写 crate::util::X）
  │   ├── scan.rs           扫描与游标：Cursor<'a> + scan_stop（尖括号已配对，仅剩 -> 守卫）
  │   └── diagnostic.rs     统一 compile_error_str(msg, span) / compile_err! / compile_err_at! 用于编译期诊断（ident-span 方案：只盖 compile_error 关键字）
  ├── parse/                解析层
  │   ├── mod.rs            入口：parse_primitive + `@` 引用解析（119 行）
  │   ├── chain.rs          运算符链解析：`-`/`^` 优先级攀爬（parse_item / parse_operand）
  │   ├── primary.rs        主类型：分组、泛型实参（含数组分发）、splat、前缀
  │   ├── trailing.rs       尾随 `{body}` / `where{...}` 拆分 + wrapper 附着
  │   ├── parse_atom.rs     原子层解析：属性 / fn / 前缀 / 范围 / 分组 / 列表
  │   └── generic.rs        泛型解析：parse_generic / parse_angle_bracket_contents（尖括号组即 delimiter![<>]）
  ├── preprocess/           预处理层（token 重写器，一个趟一个文件；mod.rs 聚合 re-export）
  │   ├── mod.rs            delimiter! 分隔符拼写宏 + 管线：angle_collect → expand_consts → expand_tokens（#name 指令展开）→ where_process
  │   ├── directives/       `#` 指令系统：#fill / #delegate / #blanket + 开放扩展（name_list / trait_items / delegate_args / blanket / blanket_wrappers）
  │   ├── consts/           `@` 常量系统：内置类型族（@u*/@i*/@f* + @scalar/@num + @u8..u128/@i8..i128/@f32..f64 范围）+ batch_trait! 自定义定义段 + where 选择器（@all_fresh / @N..M 放行）（table / expand / ctx）
  │   ├── empty_generics.rs `A<>` 照抄展开（形参渲染用合并后的 bound）
  │   ├── where_process.rs  裸 where 改写：`where 谓词 {body}` → 旧式 `where{谓词}`
  │   └── angle.rs          尖括号组：入口 None 组扁平化 + `<...>` 配对为组（输出侧还原），parse 层不再管 <> 深度
  ├── ast/                  AST 层
  │   ├── mod.rs            struct Ty { span, kind: TyKind }（TyKind 19 个变体，含 Error）+ Op 优先级定义；span 放 Ty 层、贯穿 apply 产物
  │   ├── fresh.rs          fresh 名协议（`_Param_*_BatchGen_` 常量 + 生成/构造/解析三函数）
  │   └── types_render.rs   AST 渲染：ToTokens impl for Ty + params_to_tokens 系列
  ├── apply/                运算层
  │   ├── mod.rs            Apply trait（默认 apply 做右操作数结构化分发；全部 Ty* 子类型实现 Apply）
  │   └── apply_tuple.rs    元组与容器运算符 + 元组展开（^N / 笛卡尔积 / 范围 / fresh 泛型）
  ├── codegen/              代码生成
  │   ├── mod.rs            extract_impl_parts → 后处理 → hoist_type_params → generate_impl（含 where 谓词附加与引用检查）
  │   ├── impl_parts.rs     ImplParts 结构 + TyKind 变体遍历（extract / hoist）
  │   ├── postprocess.rs    ImplParts 上的 trait 泛型替换（`From<bool>`：指令 body 里 `value: T` → `value: bool`）
  │   ├── top_level.rs      顶层宏注入（`{! ...}`——spec 主体合并 + 宏输入重写）
  │   ├── fresh.rs          fresh 名清扫（`_Param_{g}_{i}_` → 每个 impl 的 `_Param_0..N_`）+ `@N`/`@g_i` 引用校验（目标类型 / trait 实参）
  │   └── where_at.rs       `@` where 谓词解析（`@N`/`@g_i`/`@all_fresh`/`@N..M`）
  └── testing/              测试基建（cfg(test)）
      └── fuzz.rs           proptest：随机 token 喂真实宏入口（expand_attr_macro），承诺不 panic
```

## 解析流程

**token 流 → const 展开（`@` 常量：内置 + batch_trait! 自定义表）→
angle_collect 配对尖括号组 → 指令预处理（每条指令展开为 0..n 个 token：既有
指令恰一 `{...}` 组，`#blanket` 多段 spec）→ where 裸写改写 → `A<>` 照抄
→ Cursor 扫描取切片 → parse_item 优先级攀爬（`^`/`-` 经 `Apply` 组合：
右操作数结构优先分发）→ Ty AST → 工作清单摊平并列列表 → 逐叶子 generate_impl**

### 预处理顺序：`@ <> # where`（宏元层最外）

- `@` 常量展开（纯词法替换）是**最外一趟**，先于 `<>` 配对与指令：
  展开产物可能含扁平 `<...>`（如 `@map = HashMap<u32, String>` 的值、
  嵌套 `@outer = Vec<@inner>`），须由后续 angle_collect 统一配对；
- 反序（`<>` 先于 `@`）的后果：`Vec<@inner>` 的 `@inner` 被配对进
  尖括号组，而 expand_consts **刻意不进入 `<>` 组**（`delimiter![<>]`
  与真实 None 组展开值相同不可同臂区分）——`@` 残留到输出、编译报
  `found '@'`（0.6.1 实测修复）；
- 能力矩阵：`batch_impl`/`batch_impl_only` 支持内置 `@` + `<>` +
  `#` + where；`batch_trait!` 支持自定义 `@` + `<>` + where
  （指令 `#` 需要 trait 定义作签名真相源，函数式宏拿不到）。

### 关键设计决策

- **尖括号组**：proc-macro2 只对 `()`/`[]`/`{}` 分组，`<>` 是扁平 Punct。
  `angle_collect` 在入口一趟把 `<...>` 配对为 `delimiter![<>]` 组（`->` 箭头的
  `>` 不参与），下游解析不再跟踪 `<>` 深度；输出侧 `render_angles` 还原为
  扁平 `<...>`。`angle_collect` 是**破坏性**的（已配对组再次收集会被当真实
  None 组扁平化），故只做一次。
- **delimiter! 宏**：`Delimiter::None` 在本 crate 有两种语义——`delimiter![<>]`
  （尖括号组载体）与 `delimiter![none]`（真实透明组，宏变量展开产物）。二者
  展开值相同，不可在同一条 match 中作两个臂。proc-macro crate 禁止
  `#[macro_export]`，故宏置于 `preprocess` 顶部经 `#[macro_use]` 导入 crate 根
  （文本作用域要求其声明先于所有使用者）。
- **where 谓词继承**：trait 级 where 子句中**单一形参谓词**（`T: Clone`）合并进
  `TraitParam.bound`（内联 + where 拼接），**其余谓词原样透传**到 impl 的
  where 子句。引用收集在 **syn AST** 上做（`syn::visit`）：单段路径与泛型实参
  是形参引用位置；`::` 后的路径段（关联类型名）、关联类型绑定名、
- **splat `*` 前缀**：`*[...]` / `*(...)` 把容器/生成器摊平进外层列表——parse/apply/expand 全程的**整体**——只在 codegen 后处理摊平成元素（`expand_splat_elems` Ty 结构层——`TyTuple` 元素与泛型/trait 实参经 `expand_tp`，因 `TyTypeParam` 的 params 现为 `Box<Ty>`；spec 列表位置的 splat（`[*(A),*(B)]`）在 expand 阶段作为 impl 列表生成摊平）。`TySplat` 是镜像来源
  括号的枚举：`TySplat::Array`（集合——左操作数分配 `^T`，对标 `TyArray`）vs
  `TySplat::Tuple`（列表——追加/元组幂，对标 `TyTuple`）；左操作数
  `apply_help` **委托镜像容器**再包回结果，splat 保持到消费
  （实现 `X^*[A,B]^T` = `X<A^T,B^T>` 单 impl）。右 splat 操作数同样保持整体
  （`T^*(A,B)` = `T<*(A,B)>`，仅在 codegen 展开成 `T<A,B>`）。**组内孤立 splat 解析为容器、splat 作为一个元素保持**——`(*(a,b))` = `( *(a,b) )`、`[*(a,b)]` = `[ *(a,b) ]`——splat 元素只在 codegen 展开（渲染结果 `(a, b)` / `[a, b]`），一条代码路径、无按定界符的特例。**合法位置**：splat 是"参数位置列表"（泛型实参/元组/数组元素/泛型声明/fn 参数/spec 列表）；裸 splat 作 **where 谓词主体**在 codegen 明确拒绝（`*(A,B): Trait` 无定义语义——谓词是约束不是列表），谓词内部 splat（`X: Trait<*(A,B)>`）与元组谓词（`(*(A,B)): Trait`）合法。**splat 只展开一层**：元组是类型、作为单元素保持
  （`*((a,b),)` = 一个 `(a,b)` impl），数组/嵌套 splat/生成器/组摊平。
  **元组 splat 的 `^N` 幂把每个笛卡尔组合包回 splat**——`*(A,B)^2` =
  `[*(A,A),*(A,B),*(B,A),*(B,B)]`——右 splat 链把组合摊平进容器
  （`X^*(*@u*)^2` = `X<u8,u8>`/`X<u8,u16>`/...——`X<@u*,@u*>` 的重复列表
  简写；`*(A,B)^2` 单独作目标摊平成重复，E0119）。**泛型实参内的 splat 幂**
  （`Frac<*(*@u*)^2>`）——幂结果（`TyArray([*(u8,u8), ...])`）进入 params 后
  在 `expand` 的 Generic 分支分发成逐对 impl（36 个，与右 splat 链等价）；
  字面数组实参（`T<[A,B]>`）同样进 params 成 `TyArray`——数组实参分发统一在
  `expand` 的 Generic 分支（唯一权威），parse 层 `has_array_arg` 已删。
  HRTB binder（`for<'a>`）天然排除；const 泛型实参 / 数组长度经 `visit_expr`
  收集。`impl_names` 中 `const N` 归一如 `N` 以匹配引用检查。

## 语法域隔离

DSL 由三个**互不渗透的语法域**组成，各域记号自洽、语义独立：

| 域 | 记号 | 语义 | 由谁解析 |
|----|------|------|----------|
| **类型域**（spec 表达式） | `^`/`-`（同一 apply 的两种结合性：右嵌套/左累加）、`[...]` 列表、`(...)` 元组、`*[...]`/`*(...)` splat、`<...>` 泛型、`where{...}` 后缀、附着 `{body}` | 描述类型矩阵，每个格子生成一个 impl | `parse/` + `apply/` + `codegen/` |
| **指令域**（`#name{body}` / `#fill(args)` / `#delegate(args)` / `#blanket(@all){包装}` / 开放扩展） | 参数列表内 `,` 分隔、`-name` 排除项、`@all` 系列标记 | 从 trait 定义抄签名 / 批量填 body / 委托调用 / 覆盖式委托 | `preprocess/`（`parse_names_from_tokens` 独立解析，DSL 解析不进入） |
| **宏元层**（`@` 常量） | `@u*`/`@scalar` 名字族、`@u8..u128` 范围族、`batch_trait!` 前导 `@name=值;` 自定义段 | 类型矩阵命名复用；词法替换为列表后走原管线，不参与任何域内解析 | `consts.rs`（`angle_collect` 后、指令预处理前） |

### 隔离规则

- **同记号、分域、各义**：`-` 在类型域是 apply 链接（`HashMap-K-V` = `HashMap<K, V>`），
  在指令域是排除记号（`#fill(@all,-foo)`）——两域解析互不进入，语义永不冲突；
- **域边界即模块边界**：类型域解析（`parse_item` 优先级攀爬）永远不递归进入
  指令参数；指令预处理（`expand_tokens`）只展开 `#` 指令，不解释 DSL 运算符；
  `@` 常量（`preprocess/consts/`）只做词法替换，不进入任何域；
- **透传守卫统一**：`ident![...]` 宏体与 `#[...]` 属性内的内容是任意 Rust，
  四个递归入口（`angle_collect` / `expand_consts` / `expand_tokens` / `where_process`）一律不进入，
  判定收敛在 `scan::bracket_is_passthrough`（0.5.7 曾因一处守卫缺失误展开
  `#[...]` 内的 `#name` 指令）。
- **泛型实参的域分裂**：binding（`Item = u32`）与 bound（`T: Clone`）只属
  trait 路径（`Conv<Item = u32> X`）与泛型声明（`<T: Clone> Foo`）——具体
  类型的实参是纯类型列表，遇 `=`/`:` 报定向错误（`parse_angle_bracket_contents`
  的 `allow_special` 门控；此前 bound 被静默丢弃、struct binding 渲染非法代码）。

### 附着语义

指令展开产物分两类：**单组产物**（`#name`/`#fill`/`#delegate`/开放扩展的
`{...}` 组）可附着到类型后（`T {body}`）或独立成 spec；**多 token 产物**
（`#blanket` 的完整 spec 段）自含泛型/目标/委托，只能独立成 spec，附着
无意义。

### 扩展准则

新语法只能**在既有域内延伸既有机制**（如 `^`/`-` 系补充差集、指令域补充新
指令、宏元层补充新常量），不得跨域复用记号、不得改变既有记号的域内语义。
`@` 绑定与 `#blanket` 均遵循此准则：前者是宏元层纯词法替换，后者是指令域
内 `#delegate` 的自动化形态。

### 宏元层完整化：`@` 是唯一宏元记号

- **`#` 只剩指令名一种格式**：`#all` 系范围标记全部迁移到宏元层
  （`@all` 系）——选择（选哪些 item）是宏元层操作，动作（填体/委托/覆盖）
  是指令——`#fill(@all)` / `#fill(@all, -[a,b])`；
- `@all` 系展开为 **Bracket 组**（`[a,b,c]`，与 `@u*` 形态统一）后走
  指令参数解析——指令参数因此天然支持手写 `[a, b]` 与 `-[a, b]` 排除；
- **trait 感知常量**：`@trait`（batch_impl=本地名、batch_impl_only=外部
  路径；**batch_trait! 段级**——分段后逐段替换为本段 trait 路径，支持
  `@type_t=<T>@trait<T>` 跨段打包复用；try_expand_at 返 None 原样保留防
  懒递归死循环）、`@all` 系（batch_impl/only 专属，batch_trait! 报错）、
  `@Cow`（batch_impl/only 专属）：
  - `@all` 系 → 按 trait 定义选 item 的 Bracket 组（含 required/default 与
    receiver 过滤：`@all_ref_methods`/`@all_value_methods`/`@all_static_methods`）；
  - `@Cow` → `Cow<'_>` + 固有约束谓词（deref target = `T::Owned` 的
    打包，与砍掉的裸类型名常量不同类——携带约束才有复用价值）；
- **`@0` 位置引用**：where 谓词通用（codegen 渲染时 `@N` → impl 泛型第 N 位、
  `@trait` → trait 名——元组 `()^2 where{@0: Clone}` 与普通 spec 可用）；
  blanket 包装 where 中 `@0` 特指目标泛型（fresh 名——**同样由 codegen 统一
  解析**：blanket 的 fresh 是唯一 fresh，`@0` 索引到它；预处理只替换 `@trait`）；
  expand_consts 不进入 Brace 组（where 组透传），`@N` 恰好在消费点替换；
- **`<>` 只留名字**（blanket 生成的 spec 泛型只取 ident，const/lifetime
  原样）：`T: Trait` 与包装谓词并列进 where——合并 = 零分析 token 拼接
  （required ∪ default = all 同理）。blanket 的 `T: Trait` 因此与包装谓词
  天然并列；trait 形参 inline bound 由 codegen 继承逻辑放回 impl 泛型
  （不重复转移）。

### 指令统一形态：`#指令(范围){内容}`

所有内置指令都是同一形态的实例——**指令名 + 范围 + 内容**：

| 指令 | 范围（作用于谁） | 内容（怎么处理） |
|---|---|---|
| `#name{body}` | 单个 item（按名取） | 该 item 的实现体 |
| `#fill(范围){body}` | item 集合（`@all`/`@all_methods`/`@all_constants`/`@all_types`/`@all_required*`/`@all_default*`/`@all_ref_methods`/`@all_value_methods`/`@all_static_methods`/名字列表/`-name` 排除） | 统一实现体 |
| `#delegate(范围){target}` | 方法集合（`@all_methods` 等） | 委托目标表达式 |
| `#blanket(范围){包装列表}` | impl 层（整个 trait × 包装类型矩阵） | 覆盖式委托 + 包装深度（实例方法经 deref、静态方法经泛型 `t` 转发） |

- **范围**轴已覆盖：单 item → item 集合 → impl 层（粒度递增）；
- **内容**轴已覆盖：填体 → 委托 → 覆盖（处理方式递增）；
- 参数域统一由 `parse_names_from_tokens` 解析（`,` 分隔、`@all` 系标记、
  `-name` 排除），DSL 解析不进入；
- **新指令 = 在形态空间内选新的（范围，内容）组合**——现有四指令已把
  两个轴的高频组合占满；新组合须满足"作者自实现成本高"（固定模板不值钱）
  才会被采纳（`#deref` 因此被拒：`#delegate(@all_methods){self.0}` +
  `#Target{Inner}` 组合已覆盖且零新语法）。

## 错误机制

所有 DSL 语法错误均通过 `compile_error!()` 输出友好的编译错误，**永不 panic**。
两层分工，不合并：

**嵌套深度护栏**（0.6.1）：嵌套组（`[[[...]]]`）与嵌套尖括号（`Vec<Vec<...>>`）
超过 128 层报「嵌套深度超过 128 层」而非栈溢出（v0.1 承诺恢复；`angle_collect`
在配对时计数，`MAX_NEST_DEPTH = 128`）。

**span 诊断**（0.6.2）：每个 `Ty` 节点携带源 span（`struct Ty { span, kind }`），
`Ty::apply` 单点取 span 并在组合子输出中贯穿——`apply` 内错误指向左操作数位置。
`compile_error_str(msg, span)` / `compile_err_at!(span, ...)` 接显式 span。
**ident-span 方案**：`compile_error!` 只给关键字标识符盖目标 span、其余保持
call-site——全 token 带 span 时 rustc 会把错误当作 item 位置的用户代码
（"macros that expand to items must be delimited..."）。
**平台限制**（rustc 行为，宏侧不可修）：属性宏输入顶层 token 精确、组内 token
退化 call-site、`Err` 返回错误显示宏调用行——精确 span 只出现在 Ok 输出的
`Ty::Error` 路径（parse/apply）。

- **DSL 解析层**（parse/apply/codegen）：`Ty::Error` 变体在 AST 链中透传
  （链式组合中途失败需要信号值），最终经 ToTokens 输出 `compile_error!`；
- **入口层**（preprocess/expand）：`Result<_, TokenStream>` 经 `?` 传播，
  由 `util/diagnostic.rs::compile_error_str` 统一构造。

## 测试矩阵

四层：

| 目录        | 文件            | 用途                                                                                                                                                                         |
|-------------|-----------------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `examples/` | `quickstart.rs` | 可运行的 DSL 主特性 demo（`cargo run --example quickstart`），14 段覆盖基础→复杂场景                                                                                         |
| `src/`      | `fuzz.rs`       | proptest 属性测试：随机 token 序列喂 `where_process` / `parse_item`，验证"不因作者输入 panic"（`cargo test --lib`）                                                          |
| `tests/`    | `dsl.rs`        | 50 个 `#[test]`，覆盖核心特性的语义回归（含 where 子句继承、外部路径前缀、宏调用边界、`unsafe fn` 类型、列表减法 `-`、`A<>` 与同名继承、@all 状态/receiver 过滤、blanket 静态委托） |
| `tests/`    | `regression.rs` | 26 个 `#[test]`，覆盖 dsl.rs 未触碰的 corner case：嵌套 `>>`、路径类型、const 泛型、生命周期、dyn + Send、路径前缀、数组/切片 builder、`batch_impl` vs `batch_trait!` 一致性 |
| `tests/`    | `ui.rs`         | `trybuild` UI 测试：31 个 `compile_fail` fixture 锁定诊断措辞 + 1 个 `pass` fixture |

运行：

```bash
cargo run --example quickstart       # 主特性 demo
cargo test --lib                     # 单元测试 + fuzz
cargo test --test dsl --test regression   # 功能与回归测试
cargo test --test ui                  # 诊断 UI 测试
# 重新生成 UI 快照：
TRYBUILD=overwrite cargo test --test ui
```

## 发布流程

1. `CHANGELOG.md`（作者视角）与 `docs/dev-changelog.md`（开发者视角）各记
   一条
2. `cargo package` 验证打包（docs/ 目录随 git 跟踪自动入包）
3. `cargo publish`
4. `git tag vX.Y.Z && git push origin vX.Y.Z`
5. `gh release create vX.Y.Z --notes-file <notes>`

