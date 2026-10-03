# batch-impl 内部架构

**v0.10.0 — 开发中（未发布）。** 这是 0.9.7 之后的下一版本，包含 `@Self`、const/函数指针匹配、形状替换修复、限定 trait 的异步及方法泛型 blanket 转发、`@all_fresh` 移除与文档更正；次版本边界隔离这些刻意的破坏性变更。见 [CHANGELOG](CHANGELOG.md)。

本轮开发还将命名范围端点与 Rust 统一（`..` 排除、`..=` 包含），允许空指令作用域与一个尾逗号，并在差集运算前验证所有成员名；关联类型的 impl 声明与 GAT 投影实参分别生成。这些变更复用运算符字典、共享名字列表解析器与泛型名字提取函数。

delegate body 新增局部后缀 `receiver.#call`：出现可识别的标记时使用完整
方法体，否则保持目标表达式形式。扫描保留 token，继续支持重复块 DSL，
隔离宏、属性与内嵌 item；局部参数投影保留参数绑定和求值顺序。两种形式
均显式转发方法的类型/const 泛型，不扩自动 async 适配。

公开 `*T` 语法现构造参数包：映射与数字生成在 `apply` 完成，结构物化在
普通 Rust 生成前消费包槽位。旧镜像容器 Splat 节点和 codegen 展开器已移除。
固定 Rust 前缀保留结构化类型子节点，包括生命周期引用与 ABI 函数指针。

**v0.9.7**（2026-08-29）——评审修复发布：黄金展开快照（`src/testing/golden.rs` + `tests/golden/`，测试体系最后一块空白——最终渲染输出与 `BLESS=1` golden 文件锁定）、展开开销实测（`src/testing/perf.rs`，proc-macro2 层计时真实管线）、`rust-2024-feature.md` 取消跟踪（此前被打进每个 `.crate`）、Windows（MSVC）CI job（`test-windows`）、impl entry / shape 诊断精确 span（`syn::Error::span()` / leaf token span / 载体 span）、`is_impl_template` 去重归入单一权威、impl entry 提取 `chunks_to_streams()`、入口单次解析（首语义 token 扫描）。0.9.6 的入口架构不变。

**v0.9.6**（2026-08-27）——**ItemImpl 入口追上 attr 入口**：impl entry（`#[batch_impl(spec)] impl ...`）现在共享完整 DSL——其管线运行 `impl_process` → `mark_varseg` → `expand_consts`（`ConstCtx::ItemImpl { trait_path }`；内置族 + `@trait` → impl 自己的路径）→ `angle_collect` → `reject_directives` → `where_process`，然后按形状冒号拆分分发（`entry/impl_entry.rs::expand_one_spec` → `expand_shape_form` / `expand_direct_form` / `expand_leaf`）。spec 层原样复用 attr 入口的机制：矩阵源经 `collect_spec_leaves` 解析（块模型——每容器 `impl{...}` 模板成为 `TyWithImpl` 附件逐 leaf 拆出、`where{...}` 成为 `TyWithWhere` 逐 leaf 提取并共享谓词切分；只有模板区在 token 层剥离 `where`——模板必须保持 syn 类型）；生成器经 `extract::hoist_type_params` hoist；fresh 由 `FreshCtx` 命名、`range_refs::expand_range_refs` 解析；`@N..` where 选择器经 `where_at::resolve_where_predicates` 解析；body 的 `fresh!(...)` 标记（`impl_spec.rs::expand_fresh_marks`）复用 `repeat::expand_repeat_blocks` + `substitute`，隐式段绑 fresh 列表。impl entry 特有的 codegen 收敛为模板匹配（`codegen::match_shape`）、槽替换（`apply_mapping`）、hoist 的 fresh 泛型与 `assemble_impl`（impl_spec.rs）。另有：运算符字典 `util/punct_ops.rs::read_op` 成为多字符运算符形状（`..` / `..=` / `->` / `::`）的唯一权威——`scan_stop` 的三个守卫删除、`::` 识别收敛；重复的载体提取 join 去重（`tokens_to_string` / `carrier_inner`）。

**v0.9.4**（2026-08-25）——blanket 委托与 `#delegate` 改名工作（来自用户与 `auto_impl` / `delegate` / `impl-trait-for-tuples` / `fortuples` / `trait-gen` 的手动并排实测）：`#blanket` GAT 投影（`type Iter<'a> = <T as Trait>::Iter<'a> where Self: 'a`——GAT 自身参数穿过投影，`blanket.rs`）、裸 `Self` 参数/返回检测（`blanket_helpers.rs::sig_refs_bare_self`；`Self::Assoc` 返回放行）、`@?` 非 Sized 后缀（`Box@?` → `where{T: ?Sized}`，`blanket_wrappers.rs`——字段名 `is_unsized`，`unsized` 是 edition 2024 保留字）；`#delegate` 改名 `foo = call_foo`（`dispatch.rs::expand_delegate`——`=` 拆出改名映射，名字列表解析首现保留去重使 rename/`@all` 重叠合并、二次改名报错）；可读 fresh 名 `P0, P1, ...`（`codegen/fresh.rs::display_name`——`FreshCtx` 一次性分配、撞名按电子表格式字母后缀逃逸 `P0A`/`P0B`...，编号从不漂移）；生成的诊断卫生化 `::core::compile_error!`；`X<>` 在 `+` 连接的 bound 列表内同步（`sync.rs::sync_bound_ty`——结构化 `TyBoundList` 逐元素同步）；fresh 范围占位符在 impl body 内重新展开；repeat 块轮间分隔符 + fresh 数驱动 cursor-only 块 + fresh 绑定开关 `impl{@0..}` 与 `@@N` 名称引用（`repeat.rs` / `repeat_drivers.rs` / `extract.rs::parse_fresh_switch`）；空元组折叠精确化（`range_refs.rs::fold_empty_tuple`——仅顶层含范围占位符）；

**v0.9.3**（2026-08-22）——**生成式 Fn 类型**：`Fn` / `FnMut` / `FnOnce`（以及裸 `fn`）带真实参数列表结构化解析（`ast/types.rs::TyFn` + `FnKind`，`parse_atom.rs` / `parse/blocks.rs`），生成器可跑在内部（`Fn()2` → `Fn(P0,P1)`；空格形态 `Fn()N` ≡ `Fn.().N`）；`dyn` / `for<'a>` 包装结构化（`TyWithDyn` / `TyWithFor` 保持内部类型结构化）——生成器穿透 trait 对象与 HRTB；**bound 生成器**按元数范围分发（`codegen/bound_gen.rs`——每个元数一个 impl、bound 钉死、目标 `@0..` 对照该 impl 的 fresh 列表重开）；`(@0..)` 无逗号范围元组；从目标泛型实参 hoist fresh（`extract.rs::hoist_type_params` 显式递归进 `TyGeneric` 参数）；裸 `where A: Clone` 无需 `{}`；空格形态生成器拼写；`@all_fresh` 弃用（写 `@0..`）；`@Cow` 文档化为 `#blanket` 专属包装常量；

**v0.9.2**（2026-08-21）——`@N..` / `@N..M` fresh 范围在 parse 时折叠为单 token 占位符（`_Param_{N}_With[_M]_BatchGen_`，`ast/fresh.rs`）、codegen 时对照 impl 的 fresh 列表重新展开（`codegen/range_refs.rs::expand_range_refs`）——范围现在可以在任何单个 `@N` 能出现的位置使用（where 谓词、`<>` 泛型实参、impl 泛型声明、元组目标）；**组内范围** `@L_N..` 在单个生成器组内切片；变长段在元组模板中自动补尾随逗号（`preprocess/varseg.rs`）；0.9 之前 changelog 条目恢复当时的 `^` 运算符；

**v0.9.1**（2026-08-21）——稳定性发布：类型起始运算符诊断（`+A` 不再静默生成 0 个 impl；`!` 前缀不再吞掉尾随 `{...}` body——`parse/space.rs` 附件守卫）、`self` 文档化为恒等前缀（矩阵中的裸类型占位）、codegen `X<>` sync 抽入 `sync.rs::sync_impl_parts`、passthrough fn 块合并为 `passthrough_block`；文档稳定性修订（zh-CN 教程泄露修复、英文补 `# path::to::Trait:` 前缀与 `:N` 深度）；

**v0.9.0**（2026-08-21）——apply 运算符重命名（`.` 右结合、空格取代 `-` 作为左结合；`^`/`-` 退出类型域）+ **块模型**：DSL 是块的任意组合、`apply` 折叠，不再按位置剥离附件——parse 层重构（`parse/space.rs`：`parse_space` → `parse_dot` → `parse_block`；`parse_item` 按首 token 分流）；同名泛型声明合并进 where（`codegen::merge_dup_params`）；形状模板 `_` 通配（`shape.rs::match_ty` 匹配 `Type::Infer` / 数组长度 `Expr::Infer`，从不绑定）；`X<>` → 本 spec trait 应用（`codegen/sync_trait.rs`），开关模板（`impl{Tr<>}`）控制 body 同步，含路径限定；

**v0.8.1**（2026-08-18）——`where{...}` 尖括号配对 hotfix：`angle_collect` 现在进入 `where{...}` 谓词组（两参数 bound 不再被深度 0 逗号分裂）；代码体仍透传、`render_angles` 还原配对组；

**v0.8.0**（2026-08-18）——风格打底（移除 rustfmt 宽度上限、全库重排）+ 文档更新（示例注释英文化、测试数字更正）+ 扁平链深度护栏（`.`/`-` 链、附件链、链式类型段统一 128 层上限）+ 回退 0.7.2 误加的属性宏自定义 `@` 常量（`@name=value;` 段仅 `batch_trait!` 可用）+ **`impl{...}` shape template 形状模板**（新 `codegen::shape` 内核 + `TyKind::WithImpl` + `expand_consts` 进入模板、`where_process` 视为边界）+ **impl entry（ItemImpl 入口）**（`#[batch_impl]` 同样接受 `impl` 块；`entry/impl_entry.rs` + 顶层分流；形状模板 × 矩阵源实例化、`;` 分隔 spec、`@` 域仅 `@trait`；`where_process` 新增 `;` 停止与 `allow_end` 参数）；

**v0.7.2**——0.7.2 已发布：`@` 诊断用户语言化 + `batch_preview!` + trait 实参生成器 splat 提升 + `#blanket` 按值修复 + 属性宏自定义 `@` 常量（0.8.0 已回退）；0.7.1 已发布：定向诊断 + 单一真相源笛卡尔积（`util::cartesian`）+ 指令分发迁入 `directives/`；0.7.0：**splat** `*` 前缀（`TySplat{Tuple,Array}` 枚举镜像来源括号，完整委托 `TyTuple`/`TyArray` apply + 包回）、数组分发传播、parse 层拆分 `chain`/`primary`/`trailing`；0.6.x：预处理顺序 `@ <> # where`、宏元层完整化、`@N` fresh 引用、receiver 过滤、blanket 委托、span 诊断。

面向贡献者：模块组织、解析流程、错误机制、测试矩阵。

## 模块组织

```text
lib.rs              宏入口（#[batch_impl] / #[batch_impl_only] / batch_trait! / 测试宏）+ 模块树
  ├── entry/                入口与驱动
  │   ├── mod.rs            入口实现：expand_attr_macro / expand_batch_trait + 公共管线 run_pipeline
  │   ├── impl_entry.rs     impl entry（ItemImpl 入口）：形状模板 × 矩阵源实例化（attr 预处理子集 + `;` spec 切分 + 装配）；堆叠的 `#[batch_impl]` 是同一次派生的多个 stage（顺序即 rustc 的属性展开顺序）
  │   ├── impl_fresh.rs     impl entry 目标物化后的 fresh 上下文与谓词解析
  │   ├── impl_spec.rs      impl entry 装配：assemble_impl（item 自身属性随每个生成的 impl 带出）+ spec 辅助（parse_matrix_leaves / peel_where / find_shape_colon / split_new_gen）
  │   ├── driver.rs         共享驱动：collect_spec_leaves（先分离外层列表、保留开放扩展 DSL、物化普通目标，随后检查整 spec 上限并聚合错误）→ 逐叶子 generate_impl
  │   ├── pack_entry_tests.rs Pack 完整入口回归（cfg(test)）：声明、ABI 槽位、空集合与整 spec 上限
  │   ├── preview.rs        batch_preview!：诊断通道展开预览 + `.`/空格 误写提示
  │   ├── preprocess_test.rs batch_preprocess_test!：开放扩展协议的参考实现
  │   └── path_prefix.rs    外部 trait 路径前缀：#Path::to::Trait: 状态机解析
  ├── analyze/              trait 定义语义分析
  │   ├── mod.rs            re-export 门面（调用侧写 crate::analyze::X）
  │   └── trait_bounds.rs   TraitBounds / TraitParam（名字 + `ParamKind` + 合并后的 bound）：单形参 where 谓词合并，其余谓词原样透传
  ├── util/                 共享工具（mod.rs 聚合 re-export，引用侧写 crate::util::X）
  │   ├── mod.rs            re-export 门面（调用侧写 crate::util::X）
  │   ├── scan.rs           扫描与游标：Cursor<'a> + scan_stop + bracket_is_passthrough + is_impl_template + split_tuple_field_dot
  │   ├── diagnostic.rs     统一 compile_error_str(msg, span) / compile_error_ty / compile_err! / compile_err_at!（ident-span 方案：只盖 compile_error 关键字）
  │   ├── punct_ops.rs      多字符运算符字典（read_op——`..` / `..=` / `->` / `::`）
  │   └── subst.rs          路径感知替换器（replace_map——trait bound 继承 + 指令 body）
  ├── parse/                解析层
  │   ├── mod.rs            入口：parse_item 分流 + `@` 引用折叠（resolve_at_refs）+ parse_primitive（优先级阶梯的底层）+ `Ctx`（解析层的环境状态：被实现的 trait 名 + bound 位置标志 + 递归块深度）
  │   ├── chain.rs          优先级攀爬：parse_item / parse_operand / parse_space_chain / parse_dot_chain（两种 apply 结合性）
  │   ├── space.rs          块语法：starts_block / parse_block + bound 与返回表达式折叠
  │   ├── blocks.rs         块族：`&` 引用 / `*` 裸指针前缀或单块构包 / `@N` / 字面量与范围
  │   ├── ident_blocks.rs   ident 起始块：fn 族（含具名参数）/ dyn / for / impl{} / where{} / 普通与全局（`::`）路径
  │   ├── parse_atom.rs     原子层解析：分组 / 列表 / 范围
  │   ├── reentry.rs        新生成器求值前预留传回的 fresh 声明；路径实参与 Rust token 域不预留组号
  │   └── generic.rs        泛型解析：parse_angle_bracket_contents / split_at_depth0（尖括号组即 delimiter![<>]）
  ├── preprocess/           预处理层（token 重写器，一个趟一个文件；mod.rs 聚合 re-export）
  │   ├── mod.rs            delimiter! 分隔符拼写宏 + expand_tokens（`#` 指令扫描）
  │   ├── stream.rs         Stream<S> 类型态链——预处理顺序由类型系统强制（Raw → Marked → ConstsDone → Paired → DirectivesResolved → WhereDone → Ready）
  │   ├── consts/           `@` 常量系统——宏元层，**最外层** pass（先于尖括号配对）
  │   │   ├── mod.rs        常量系统门面 + 文件地图（re-export）
  │   │   ├── table.rs      内置族（@u*/@i*/@f*/@num/@scalar + @u8..=u128/@i8..=i128/@f32..=f64 范围）+ 仅 `batch_trait!` 的自定义前导段 `@name=value;`（入口 expand_consts / collect_user_consts）
  │   │   ├── expand.rs     单个 `@` 的识别（try_expand_at）+ 常量值引用校验（check_value_refs）
  │   │   ├── ctx.rs        ExpandCtx：一趟解析的并集来源（内置名族 + 范围族、@trait/@all/@Cow、用户表）
  │   │   ├── range.rs      范围族端点：解析 / 宽度校验 / 排他与包含端点 / 开放端解析（纯函数）
  │   │   └── value_refs.rs 常量值内部引用可见性——循环 / 前向 / 未知引用在定义处报错
  │   ├── directives/       `#` 指令系统——#fill / #delegate / #blanket + 开放扩展（#name(args){body} → 顶层宏调用）
  │   │   ├── mod.rs        指令模块门面 + 文件地图（re-export）+ reject_directives（ItemImpl 入口的 `#` 策略：裸 `#name(...)` 一律拒绝）
  │   │   ├── dispatch.rs   分发表：#name{body} / #cmd(args){body} → 对应展开（含 #fill / #delegate / 单项展开）
  │   │   ├── name_list.rs  指令实参名单（@all 标记、显式 ident 列表、`-name` / `-[a, b]` 减法）
  │   │   ├── trait_items.rs trait item 查找（#name / #fill / #delegate 的签名真相源）+ @all 族标记 spec
  │   │   ├── delegate_args.rs 哪些 syn::Pat 可原样转发进委托调用 + 调用实参收集
  │   │   ├── delegate_template.rs 局部 .#call 扫描 + 完整方法体渲染 + 卫生化参数投影
  │   │   ├── blanket.rs    #blanket：wrapper 列表 → 每个 wrapper 一个完整委托 spec
  │   │   ├── blanket_wrappers.rs blanket wrapper 列表解析（类型表达式 + `:N` 解引用深度 + 尾随 where{...}）
  │   │   └── blanket_helpers.rs blanket 辅助：`Self` 返回检测、`@0` 目标标记、成组 trait 路径渲染、wrapper-where 的 @trait 解析
  │   ├── varseg.rs         变长段（`ident@..`）：模板标记、解码 API、残留后置条件诊断
  │   ├── where_process.rs  裸 `where` / 裸 `impl` 收集（kw_process）——一个收集器、两条边界规则
  │   ├── empty_generics.rs `A<>` 照抄展开（形参渲染用合并后的 bound）
  │   └── angle.rs          尖括号组：入口 None 组扁平化 + `<...>` 配对为组（输出侧 render_angles 还原），parse 层不再管 <> 深度
  ├── ast/                  AST 层
  │   ├── mod.rs            门面（子模块 re-export）
  │   ├── types.rs          struct Ty { span, kind: TyKind }——TyKind 共 **27** 个变体（含 Error、Pack 与结构化单槽宿主 Prefixed）+ TyTypeParam / TyParams；span 放 Ty 层、贯穿 apply 产物
  │   ├── op.rs             Op 优先级阶梯 + MAX_EXPAND + count_leaves（展开质量计数）
  │   ├── param_kind.rs     ParamKind（Type / Const / Lifetime）：**唯一权威**回答"这个名字是哪类参数"——DSL 名字 token 流（of_name）与 syn::GenericParam（of_generic_param）归一到同一套词汇，另含 `const` 关键字剥离（bare_name）；反复咬人的规则：名字只是**以** `const` 开头（`constant`）就是普通类型参数
  │   ├── types_from.rs     `to_ty()` 构造器
  │   ├── types_render.rs   AST 渲染：ToTokens impl for Ty + params_to_tokens 系列
  │   ├── types_visit.rs    map_children（唯一遍历权威）+ 展开辅助
  │   ├── visit_tests.rs    遍历回归（cfg(test)）：容器、声明、错误与展开质量
  │   ├── expand.rs         并列列表展开（Expand::Many / Expand::Leaf），保持 Pack 层次
  │   ├── materialize.rs    最终槽位/候选收集，携带声明并对笛卡尔组合限量
  │   ├── materialize_hosts.rs 普通类型宿主重建；单槽与多槽消费边界
  │   ├── materialize_params.rs 泛型实参、声明名/bound 与关联类型绑定槽位
  │   ├── materialize_tests.rs 物化回归（cfg(test)）
  │   ├── fresh_counter.rs  每 spec 的组号分配与传回声明的预留
  │   └── fresh_protocol.rs fresh/段槽载体协议（`FreshRef` + `@{...}` 载体 + fold_flat_refs；spell/parse 双向）
  ├── apply/                运算层
  │   ├── mod.rs            Apply trait：默认 `apply` 做右操作数结构化分发（Array/Group/WithCode/WithImpl/WithWhere/WithType/Range/Error 通用处理；其余落到 `apply_help`，故其右操作数必为普通类型）；各子类型实现 `apply_help`，`impl Apply for TyKind` 按变体转发（WithDyn/WithFor/Prefixed 下沉进内层；Lifetime/BoundList 直接报错）；Ty::apply 单点取 span
  │   ├── apply_tuple.rs    元组与容器运算符 + 元组展开（.N / 笛卡尔积 / 范围 / fresh 泛型）
  │   ├── pack.rs           公开 Pack 内核：保持整行的 map_task + 数字包生成 + 包运算
  │   ├── pack_limits.rs    Pack 输入深度/质量检查及分配前生成成本检查
  │   ├── pack_tests.rs     Pack AST 回归（cfg(test)）：分组、行身份、元数据与声明顺序
  │   ├── pack_limit_tests.rs Pack 宿主、元数据与资源边界回归（cfg(test)）
  │   └── star.rs           `*` 前缀算子：`Star::star` —— 按节点种类把操作数打开成 Pack 的那张表
  ├── codegen/              代码生成
  │   ├── mod.rs            generate_impl：开放扩展输出，或从已物化目标生成普通代码
  │   ├── extract.rs        Ty → ImplParts（extract_impl_parts / substitute_trait_generics / hoist_type_params / split_impl_attachments）
  │   ├── pipeline.rs       generate_parts：一个 ImplParts → 一个 impl 的**阶段顺序权威**
  │   ├── generics.rs       merge_dup_params / inherit_trait_bounds / hoist_bound_fresh（参数名归一来自 `ParamKind::bare_name`）
  │   ├── sync.rs           `X<>` 同步（trait 实参 / bound / `impl{Tr<>}` 开关模板的 body 选项）
  │   ├── where_at.rs       where 谓词 `@` 解析（`@N` / `@g_i` / `@N..M`）+ 拒绝在 Rust where 谓词中使用类型域 `*` 语法
  │   ├── where_at_tests.rs where 谓词 `@` 引用测试（cfg(test)）：`@N` / `@g_i` 位置、`@N..M` 范围、组引用、开放范围
  │   ├── validate.rs       目标类型 / trait 实参里的悬空 `@` 校验
  │   ├── fresh_naming.rs   FreshCtx：fresh 显示命名（文档序 P0..、撞名以 P0A/P0B 逃逸）+ 碰撞集来源清单（`used_ident_set` / `collect_used_surfaces`）+ 共享 `@N` 诊断
  │   ├── range_refs.rs     `@N..` 占位符重开 / 声明展开 / 空元组折叠
  │   ├── range_worker.rs   范围引用展开测试（从 range_refs.rs 拆出以守住单文件预算）
  │   ├── shape.rs          形状模板内核：Mapping + VarSeg + ShapeError
  │   ├── shape_args.rs     路径/const 实参与函数指针匹配；依据声明核对参数种类
  │   ├── shape_tests.rs    形状内核回归（cfg(test)）：字面约束、参数种类与函数契约
  │   ├── match_ty.rs       模板 vs 叶子的结构递归（覆盖每种 syn::Type 形态；`_` 通配；const 参数数组长度可绑定）
  │   ├── repeat.rs         `@(...)..` 重复块 + token 预算
  │   ├── repeat_drivers.rs 段驱动 + 逐轮替换
  │   ├── repeat_tests.rs   `@(...)..` 重复块测试（cfg(test)）：变长段轮次、`@ident` 元素拼接、`@N` 游标替换
  │   ├── top_level.rs      顶层宏注入（`{! ...}`——spec 主体合并 + 宏输入重写）
  │   └── render.rs         collect_shape_mapping + render_impl
  └── testing/              测试基建（cfg(test)）
      ├── fuzz.rs           proptest：随机 token 喂真实宏入口，承诺不 panic
      ├── golden.rs         黄金展开快照（`BLESS=1` 重写）
      ├── perf.rs           展开开销测量
      └── mod.rs            GuardAlloc 分配守卫（256 MiB）
```

## 解析流程

**token 流 → const 展开（`@` 常量：内置 + batch_trait! 自定义表）→
angle_collect 配对尖括号组 → 指令预处理（每条指令展开为 0..n 个 token：既有
指令恰一 `{...}` 组，`#blanket` 多段 spec）→ where 裸写改写 → `A<>` 照抄
→ Cursor 扫描取切片 → parse_item 优先级攀爬（空格/`.` 经 `Apply` 组合：
右操作数结构优先分发）→ Ty AST → 工作清单展开外层候选 → 保留开放扩展 DSL
或物化普通目标 → 整 spec 展开上限 → generate_impl**

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

### 类型态管线（`preprocess/stream.rs`）

上述顺序由**类型系统**强制而非散文：`Stream<S>` 把 token 向量包在
一个以**不变量**命名的状态里（不是以 pass 命名），每个 pass 只作为
"可消费它的状态"上的方法存在——顺序写错就编译失败，编译器承担顺序
契约的结构性一半（本项目由轮换的 AI 评审开发、无共享记忆，散文纪律
不够）。

```
Raw ──preprocess()──▶ Paired ──expand_tokens──────▶ DirectivesResolved ──where_process──▶ WhereDone ──expand_empty_trait_generics──▶ Ready
                       │            └─reject_directives──▶（同状态；impl entry）
                       └─（batch_trait! 尾巴）──where_process──▶ WhereDone
```

| 状态 | 不变量（保证什么） |
|---|---|
| `Raw` | 原始 token（裸 `impl` 未收集、`@..` 未标记） |
| `Marked` | 变长段已标记——`ident@..` 对 `expand_consts` 不透明 |
| `ConstsDone` | `@` 已解析——产物可能含扁平 `<...>`！ |
| `Paired` | `<...>` 已配对成不透明组——**破坏性，绝不二次配对** |
| `DirectivesResolved` | `#` 已处理（`expand_tokens` 展开或 `reject_directives` 拒绝；`#[...]` 原样） |
| `WhereDone` | 裸 `where` 已改写——谓词内 `Foo<>` 安全 |
| `Ready` | `A<>` 已展开——唯一可交给 `syn::parse` 的状态 |

公共前缀是一个方法——`Stream<Raw>::preprocess(ctx) → Stream<Paired>`
（裸 impl 收集 → 变长段标记 → `@` 展开 → 配对）；它途经中间状态 `Marked`
（`ident@..` 已标记）与 `ConstsDone`（`@` 已解析——输出可能仍含扁平 `<...>`），
故上图只画入口可见的迁移。三个入口在 `Paired`
之后选尾巴（attr 走 `expand_tokens`、impl entry 走 `reject_directives`、
`batch_trait!` 直通 `where_process`）。`expand_empty_trait_generics` 只
存在于 `WhereDone`（仅 attr）——"谓词内 `Foo<>` 必须先直通"是方法可用
性规则，不是注释。

**fuzz 绕过链**（它刻意乱序直调单 pass）。自由函数保持 `pub(crate)`；
链之外的唯一守卫是 `mark_template` 的**后置条件**（`preprocess/varseg.rs`）——
其输出不得含未标记的 `ident@..`——违反时在**所有构建档**返回诊断而不是
断言（proc macro 里的 `assert!`/`panic!` 就是编译器 ICE）。它挂在消费方
输出上，因为只有那里段形状无歧义：开放范围常量的 `@` 前是 `<`/`,`/`(`，
绝不可能是 ident。`angle_collect` 的金丝雀不可行：配对**产物**与真实透明组同为
`Delimiter::None`，token 层无法区分（见 `delimiter!` 宏注）——类型态
链是它唯一的守卫。

### 两个前端：什么已共享，什么还没

**attr 入口**（`#[batch_impl(spec)] trait …` / `batch_trait!`）与 **impl 入口**
（`#[batch_impl(spec)] impl …`）是同一套 DSL 的两个前端，凡是"表驱动"的关注点都已经只有
一处：

| 关注点 | 权威（两者共享） |
|---|---|
| `@` 常量 / `#` 指令 / 裸 `where` | `preprocess/stream.rs` 类型态链（impl 入口以 `reject_directives` 替代 `expand_tokens`） |
| spec → 工作清单 | `entry/driver.rs::collect_spec_leaves` |
| 重复块 + 段替换 | `codegen/repeat.rs` + `codegen/repeat_drivers.rs`（impl 入口把标记写成 `fresh!(…)`，因为它的 body 必须保持合法 Rust） |
| 形状模板 | `codegen/match_ty.rs`、`codegen/render.rs::collect_shape_mapping` |
| fresh 显示名 + 碰撞集 | `codegen/fresh_naming.rs`（单一来源清单） |
| `@N..` 重开 / where 选择器 | `codegen/range_refs.rs`、`codegen/where_at.rs` |
| 参数分类 | `ast/param_kind.rs` |
| item 自身属性 | attr 入口：spec 的附加属性；impl 入口：`item.attrs`——两者都会发射 |
| **空** spec 列表 | 两个入口都是无操作：没有可派生的东西，item 按原样成立（impl 入口发射原 impl 块，而不是把它扣下） |
| 堆叠的 `#[batch_impl]` stage | 仅 impl 入口：rustc 先展开最外层属性，本入口把其余属性重新发射到自己派生的 impl 上，于是各步按源码顺序作用于**累积中的块**（前一步留下的槽位由后一步绑定）。入口侧无状态：顺序就是编译器的属性展开顺序，由 `tests/features/impl_entry_chain.rs` 锁定 |

**R1 第 3 阶段：完成——一个渲染器。** 两个入口现在都构造 `ImplParts`，交给
`codegen/render.rs::render_impl`，由它拥有整个 impl 块——属性、`unsafe`、头部（trait 为
`None` 时用 inherent 形态）、where 子句拼接与 body——经共享骨架
（`render_impl_block`）。关于这些槽位的任何规则只写一次；而"装配阶段功能加两遍"这一失败
模式——正是它让 impl 入口丢掉条目属性好几轮——再也写不出来。

两个入口**仍然**不同的地方是**输入映射**，且是有意的：`codegen/pipeline.rs::generate_parts`
提供 DSL 侧的 parts（带类型的 `impl_generics`、`associated_types`、带 `@N..` 重开的
`trait_generic_names`），而 `entry/impl_spec.rs::assemble_impl` 提供 token 层 parts——泛型
形参是原样 `(tokens, None)` 对（bound 就包在形参 token 里，槽位剥除逻辑留在入口）、body
是一整段流、`target_type` 保持为不透明的 `TyPrimitive` 兜底节点：渲染器**从不读取**
`target_type`——它渲染调用方给的 `target_tokens`——因此把目标解析成真实节点是没有东西能
观察到的计算（解析成功与兜底分支渲染结果完全一致，也就没有任何测试能证伪它），入口不做。
两套模型在同一个
输出上会合，而不假装是同一套模型。把入口输入彻底 AST 化只在未来某个特性必须**推理**
impl 入口的目标或形参时才有意义；今天没有这样的特性，因此"一个渲染器 + 两个适配器"就是
代码的真实形状。

### 关键设计决策

- **尖括号组**：proc-macro2 只对 `()`/`[]`/`{}` 分组，`<>` 是扁平 Punct。
  `angle_collect` 在入口一趟把 `<...>` 配对为 `delimiter![<>]` 组（`->` 箭头的
  `>` 不参与），下游解析不再跟踪 `<>` 深度；输出侧 `render_angles` 还原为
  扁平 `<...>`。`angle_collect` 是**破坏性**的（已配对组再次收集会被当真实
  None 组扁平化），故只做一次。因为组是**自洽原子**，**限定类型**需要一个显式
  判别式：以 `<...>` 组开头、且组内含**深度 0 的 `as`** 时，它是限定自身头
  （`<T as Tr>::Assoc`），不是实参列表。这个问题只在一处回答
  （`parse::split_projection`），三处提问者共用——块解析器（以
  `<T as Tr>::Assoc` 开头的类型）、ident 路径解析器（`M2 <S as Tr>::Assoc` 不得把
  该组当作 `M2` 的实参）、impl 入口的 `new-generic-decl` 切分（`<S as Tr>::Assoc`
  不得被当作声明吞掉——那会留下悬空的 `::Assoc`）。`TyKind::Qualified` 携带头
  （`QualifiedHead::Type` 或 `Projection`）与**原样**的 `::` 尾巴：尾巴是普通 Rust
  路径文本，逐 token 匹配、绝不重新解析。
- **delimiter! 宏**：`Delimiter::None` 在本 crate 有两种语义——`delimiter![<>]`
  （尖括号组载体）与 `delimiter![none]`（真实透明组，宏变量展开产物）。二者
  展开值相同，不可在同一条 match 中作两个臂。proc-macro crate 禁止
  `#[macro_export]`，故宏置于 `preprocess` 顶部经 `#[macro_use]` 导入 crate 根
  （文本作用域要求其声明先于所有使用者）。
- **where 谓词继承**：trait 级 where 子句中**单一形参谓词**（`T: Clone`）合并进
  `TraitParam.bound`（内联 + where 拼接），**其余谓词原样透传**到 impl 的
  where 子句。trait 形参的替换**按位置**而非按名字（`inherit_trait_bounds` 把
  trait 实参与 trait 形参按位置配对），且每个 `TraitParam` **携带**自己的种类
  （`ParamKind`，取自 `syn` 变体），不再由 codegen 从名字字符串反推；
  `impl_names` 里 `const N` 经 `ParamKind::bare_name` 归一为 `N`。（此处曾有
  一套 `syn::visit` 引用收集器，产物无人读取——R4 已删除。）
- **参数包 `*` 前缀**：`star_block` 只读取紧随其后的一个块，`*const` / `*mut` 优先识别为指针，随后把该操作数交给 `Star::star`（`apply/star.rs`）—— 解析器自身不含任何 `*` 语义。`star` 打开普通元组或候选列表的直接成员，穿过分组和声明载体，已有 Pack 保持不变，其余类型构成单成员包。因此 `*(A)` 是单成员包，`(*(A,B))` 是包外的透明分组，`(*(A,B),)` 才是显式元组宿主；`(@0..)` 保留范围引用元组的规则。列表序列化保留单成员逗号（`[A,]`），空候选列表写成 `[,]`，使开放扩展重解析不会把列表变成 slice 或空构造器。
- **映射与生成**：左 Pack 逐成员映射；双 Pack 运算只把右包拆成直接行一次，`map_task` 在选择候选与穿过元数据时始终把该行整体传递。嵌套 Pack 到消费前保留结构。标量应用仍把右包作为实参节点追加；`F<...>` 已是泛型宿主，物化不会对其中实参重跑 apply。包的数字应用将嵌套包消费成模板槽并复用元组生成；普通元组幂保留自身槽位：`(*(),).2` 是两个最终消费为空的槽，得到 unit；`*(*(),).2` 则生成两个 fresh 参数。声明随复制及空结果保留，不静默裁剪未使用的生成参数。
- **物化边界**：`materialize_targets` 拼接 Pack 成员并选择普通候选分支，保持候选的笛卡尔组合与声明顺序。元组成员、泛型实参、泛型声明名及无名函数参数是多槽位置；引用/指针/slice 元素、固定 Rust 前缀、函数返回值、每项 bound 和关联类型绑定值在每个分支中只能得到一个类型。声明名在展开后校验，构造类型和声明内 fresh 生成器仍有定向错误。普通元组在单槽宿主中始终是一个类型。共享驱动的 `Expand::Leaf` 分支保留开放扩展 DSL，并物化全部普通目标；整 spec 上限在汇总全部目标后检查。因此 trait 生成、impl 入口匹配与 preview 收到同一批已物化目标。标准 Rust where 子句、body、impl 模板及限定路径尾部不增加 Pack 语法。

## 语法域隔离

DSL 由三个**互不渗透的语法域**组成，各域记号自洽、语义独立：

| 域 | 记号 | 语义 | 由谁解析 |
|----|------|------|----------|
| **类型域**（spec 表达式） | `.`/空格（同一 apply 的两种结合性：右嵌套/左累加，外加裸 trait 名）、`[...]` 列表、`(...)` 元组、`*T` 参数包、`<...>` 泛型、`where{...}` 后缀、附着 `{body}` | 描述类型矩阵，每个格子生成一个 impl | `parse/` + `apply/` + `codegen/` |
| **指令域**（`#name{body}` / `#fill(args)` / `#delegate(args)` / `#blanket(@all){包装}` / 开放扩展） | 参数列表内 `,` 分隔、`-name` 排除项、`@all` 系列标记；delegate body 表达式中的局部 `.#call` | 从 trait 定义抄签名 / 批量填 body / 委托调用 / 覆盖式委托 | `preprocess/`（`parse_names_from_tokens` 独立解析 scope；`delegate_template` 改写调用标记，不把 body 作为类型解析） |
| **宏元层**（`@` 常量） | `@u*`/`@scalar` 名字族、`@u8..=u128` 范围族、`batch_trait!` 前导 `@name=值;` 自定义段 | 类型矩阵命名复用；词法替换为列表后走原管线，不参与任何域内解析 | `preprocess/consts/`——**最外层** pass，先于 `angle_collect`（其值可能含扁平 `<...>`，必须被配对看到） |

### 隔离规则

- **同记号、分域、各义**：空格在类型域是左结合 apply（`HashMap K V` = `HashMap<K, V>`），
  `-` 在指令域是排除记号（`#fill(@all,-foo)`）——两域解析互不进入，语义永不冲突；
- **域边界即模块边界**：类型域解析（`parse_item` 优先级攀爬）永远不递归进入
  指令参数；指令预处理（`expand_tokens`）只展开 `#` 指令，不解释 DSL 运算符；
  `@` 常量（`preprocess/consts/`）只做词法替换，不进入任何域；
- **透传守卫统一**：`ident![...]` 宏体与 `#[...]` 属性内的内容是任意 Rust，
  四个递归入口（`angle_collect` / `expand_consts` / `expand_tokens` / `where_process`）一律不进入，
  判定收敛在 `scan::bracket_is_passthrough`（0.5.7 曾因一处守卫缺失误展开
  `#[...]` 内的 `#name` 指令）。
- **delegate body 识别是局部的**：`delegate_template` 仅在交给 `#delegate`
  的 body 内识别 `.#call`，复用宏/属性透传守卫，并额外隔离内嵌 item 定义。
  被跳过的标记不触发完整方法体模式。闭包与内联 `const { ... }` 表达式仍属
  body 域，内嵌函数或 const item 则不属于；扫描保留 token，后续重复展开
  仍能读取自身的 DSL。
- **泛型实参的域分裂，由 `ArgsPosition` 决定**：尖括号块里允许什么，是**位置**的属性，
  不是列表形状的属性——`parse::generic::ArgsPosition` 命名了三种：**trait 应用**
  （`Conv<Item = u32> X`）与 **bound**（`T: Iterator<Item = u8>` / `dyn …` /
  `for<'a> …`）同时接受 bound 与 binding；**泛型声明**（`<T: Clone> Foo`）接受 bound，
  而那里的 binding 什么都不声明，因此报错并给出 trait 应用的写法
  （`Trait<Item = u8> Target`）；纯类型的实参（`Vec<u8>`）两者都不接受。前两种之外
  的 `=` 报定向错误（此前 bound 被静默丢弃、struct binding 渲染非法代码）。
- **唯一的解析环境上下文**：解析器唯一的状态是 `parse::Ctx { trait_name, bound, block_depth }`
  （`Copy`），从 `parse_item` 逐层按值传到 ident 块。`trait_name` 回答"这个裸头是不是
  被实现的 trait"（决定 `TyTrait` 还是 `TyGeneric`）；`bound` 回答"这条路径是否处于
  bound 位置"——由 bound 解析器设置（`parse_bound_expr`、`dyn_block`、`for_block`），
  进入嵌套实参列表时再清掉（那些块是类型）。两者**分开**才能让 `Vec<Item = u8>` 继续
  报错而 `<T: Iterator<Item = u8>>` 可解析：该标志只放宽实参门控，绝不参与头的分类；
  而**门控本身**就是上面的位置枚举。`block_depth` 单独限制 `***T` 这类
  不产生定界符嵌套的递归前缀。
- **开头的 `::` 是块，单个 `:` 不是**：因此 `starts_block` 读的是游标（它要查
  复合运算符字典）而不是单个 token，`parse_block` 多出一条全局路径分支，把头部交给
  唯一的 ident 路径解析器（`plain_ident_path`）。token 级检查无法区分这两者，而
  `T: Clone` / `fn(x: u8)` 的边界诊断恰恰依赖"单个 `:` 仍是分隔符"。

### 附着语义

指令展开产物分两类：**单组产物**（`#name`/`#fill`/`#delegate`/开放扩展的
`{...}` 组）可附着到类型后（`T {body}`）或独立成 spec；**多 token 产物**
（`#blanket` 的完整 spec 段）自含泛型/目标/委托，只能独立成 spec，附着
无意义。开放扩展自 0.6.7 起**仅顶层**：`{! m!{...}}` 前置 spec body 并把宏
调用发射到顶层；旧的内嵌形态 `T {m!{...}}`（无 `!`，输出关联项）自 0.7.2
标注弃用，保留兼容。

开放扩展协议保留 DSL 和 fresh 声明载体，不提前改成最终 Rust 名字。重入时，
`parse/reentry.rs` 在新生成器求值前预留真正传回的声明，避免复用其组号。
类型实参、谓词或正文中的引用不分配组号。随后物化与命名只使用已选分支
自身携带的声明。

### 扩展准则

新语法只能**在既有域内延伸既有机制**（如 `.`/空格 系补充差集、指令域补充新
指令、宏元层补充新常量），不得跨域复用记号、不得改变既有记号的域内语义。
`@` 绑定与 `#blanket` 均遵循此准则：前者是宏元层纯词法替换，后者是指令域
内 `#delegate` 的自动化形态。

**语法面冻结（0.7.2）**：既有记号语义受保护，常规更新只做加法、诊断精化与文档改进。刻意的破坏性变更必须设置兼容边界并明确迁移方式，开发中 0.10.0 的说明见 README 与 changelog（`@N` 稳定性承诺覆盖整个语法面）。

### 宏元层完整化：`@` 是唯一宏元记号

- **选择使用 `@`，动作使用 `#`**：`#all` 系范围标记全部迁移到宏元层
  （`@all` 系）；指令名决定动作（`#fill(@all)` / `#fill(@all, -[a,b])`）。
  `.#call` 是 delegate body 内的局部调用标记，不是新选择器或顶层指令；
- `@all` 系展开为 **Bracket 组**（`[a,b,c]`，与 `@u*` 形态统一）后走
  指令参数解析——指令参数因此天然支持手写 `[a, b]` 与 `-[a, b]` 排除；
- **trait 感知常量**：`@trait`（batch_impl=本地名、batch_impl_only=外部
  路径；**batch_trait! 段级**——分段后逐段替换为本段 trait 路径，支持
  `@type_t=<T>@trait<T>` 跨段打包复用；try_expand_at 返 None 原样保留防
  懒递归死循环）、`@all` 系（batch_impl/only 专属，batch_trait! 报错）、
  `@Cow`（batch_impl/only 专属）：
  - `@all` 系 → 按 trait 定义选 item 的 Bracket 组（含 required/default 与
    receiver 过滤：`@all_ref_methods`/`@all_value_methods`/`@all_static_methods`）；
  - `@Cow` → `Cow<'_>` + 打包谓词（`T: ToOwned + ?Sized`，以及现有的额外
    `T::Owned: Trait` 要求）；其 Deref 目标是 `T`；
- **输入 impl 上下文**：`@Self` 沿已有常量阶段复制 `ConstCtx::ItemImpl`
  的输入自身类型；trait 属性与 `batch_trait!` 没有该值。堆叠属性提供本层
  输入，无需额外阶段状态。
- **命名范围端点**：`src/preprocess/consts/expand.rs` 经 `read_op` 读取 `..` / `..=`，不再依赖 span 邻接判定端点。后续标识符位宽合法时作为显式端点；否则排他开放范围将它留给下一个 DSL 项。`range.rs` 根据排他/包含标记筛选，省略上端点时包含族最大值；反向范围与空排他范围仍报错。单成员范围由 `render_list` 发射为 `[u8,]`，保留列表语义，避免成为切片。
- **`@0` 位置引用**：where 谓词通用（codegen 渲染时 `@N` → impl 泛型第 N 位、
  `@trait` → trait 名——元组 `().2 where{@0: Clone}` 与普通 spec 可用）；
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
| `#delegate(范围){...}` | 方法集合（`@all_methods` 等） | 无 `.#call` 时是目标表达式；否则是完整方法体，各处标记原位展开为调用 |
| `#blanket(范围){包装列表}` | impl 层（整个 trait × 包装类型矩阵） | 覆盖式委托 + 包装深度（实例方法经 deref、静态方法经泛型 `t` 转发） |

- **范围**轴已覆盖：单 item → item 集合 → impl 层（粒度递增）；
- 共享名字列表解析器接受空参数、合法空选择和一个尾逗号（含嵌套列表与排除列表），前导/连续逗号仍报错。正向与负向名字都在差集运算前检查是否存在于 trait。空选择使 `#fill` / `#delegate` 不生成成员，`#blanket` 仍生成包装 impl；包装列表的逗号属于另一套语法。
- 关联类型 impl 定义去掉 trait 的结果约束，保留 GAT 参数声明与 `where` 谓词。blanket 投影经 `generic_param_names` 只传生命周期、类型与 const 的名字：`Item<'a, U: Clone, const N: usize>` 是声明，`Item<'a, U, N>` 才是应用。
- `dispatch::expand_delegate` 为两种形式共建改名表并归一化方法参数模式。
  `delegate_template::Template` 记录一次调用标记位置，按每个选中签名渲染；
  `method_turbofish` 与 blanket 转发共用。标记已经是完整调用，后接
  `.into()`、字段访问或 `()` 均作用于返回值；不自动添加 `.await`。
- 有参数时，完整方法体形式在方法内生成 `macro_rules!` 参数投影 helper：
  各臂引用原来的归一化参数绑定，每个带标记调用在实参位置展开投影。
  定义处绑定避免分支/闭包同名局部变量捕获转发参数，调用处展开避免提前
  移动或借用，并保持先接收者、后参数的求值顺序。helper 名字避开整个
  属性与 trait 内的标识符，防止后续词法形状替换误改 helper 名。
- `util::split_tuple_field_dot` 由重复替换与 delegate 扫描共用，拆开吞掉
  后续点号的元组字段整数字面量（`self.0.#call` 被词法器读成 `0.`），
  不把任意浮点字面量视为元组字段。模板扫描沿用共享 `MAX_NEST_DEPTH`。
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

该承诺现在是**机器强制**的（两条腿，均可证伪——临时违规必须让它失败）：
`lib.rs` 的 `cfg_attr(not(test), deny(clippy::unwrap_used, expect_used, panic,
unreachable, todo, unimplemented))` 家族，以及基于 `syn` 的源码守卫
`tests/no_panic/main.rs`（它还禁止用 `#[allow]` 静默该家族）。其背后的潜在类靠**构造**
而非论证兜住：**索引/切片**依赖 `Cursor` 的夹取位置不变量（`pos <= len`）与
`get()` 式访问器 / 扫描派生索引；**算术**依赖"每个用户字面量落点先校验边界、
饱和、或改用 `get()`"——`@N..M` 端点在任何 `- 1` 之前先对作用域长度校验、
range 长度在分配前算出并封顶、repeat 游标/轮次族饱和；其余计数器受 token 向量
长度约束，溢出需要 2⁶⁴ 量级的输入。栈深度由 `MAX_NEST_DEPTH` 封顶、分配由
`MAX_EXPAND` / `MAX_REPEAT_TOKENS` 封顶——这两者是 abort 而非 panic，正是这些
上限存在的原因。索引/切片这一类**已关闭**：棘轮把每个生产落点迁完（**208 → 0**——
在棘轮前修订上重测为 207 个 `indexing_slicing` + 1 个 `string_slice`，跨 35 个文件；
迁移当时的自计数为 203），随后把那些临时文件级 deny（记录该数字时为 24 个）收编为一行
crate 级 `deny(clippy::indexing_slicing,
clippy::string_slice)`，由
`tests/no_panic/main.rs::the_crate_denies_the_panic_and_indexing_families` 断言（探针已证实
该行覆盖那些从未带过局部属性的文件）。两个 lint 都要点名，因为它们是**分开**的：只写
索引那一个时 `v[0]` 与 `&v[..1]` 会被抓、`&s[..1]` 不会——探针因此找出这个盲区里唯一
的生产落点（`repeat_drivers` 的 `s[..s.len() - 1]`，现为 `strip_suffix('.')`，顺带
消掉了它的裸 `- 1`）。

**嵌套深度护栏**（0.6.1）：嵌套组（`[[[...]]]`）与嵌套尖括号（`Vec<Vec<...>>`）
超过 128 层报「嵌套深度超过 128 层」而非栈溢出（v0.1 承诺恢复；`angle_collect`
在配对时计数，`MAX_NEST_DEPTH = 128`）。

**扁平链深度护栏**（0.8.0）：若干扁平构造不产生任何组嵌套，却同样构建深 `Ty` 树，
token 层护栏看不见它们——`.`/空格 算子链（右结合 `.` 每个操作数嵌套一层 `TyGeneric`）、
链式类型段（`<T><U>...X`、`Trait<A> Trait<B>... X`、`#[a] #[b]... X`）。两者都在解析层封顶 128
（`parse_space_chain` 的单位计数与 `parse_dot_inner` 的共享操作数计数——解析层的附件计数与段深度
随块模型一起消失，因为附件链现在只是又一条块链），使下游
所有递归遍历（`map_children` / `materialize_targets` / `hoist_type_params` /
`ToTokens`）深度有界——此前约 850 个 `.` 链式单元即令 rustc 栈溢出
（STATUS_STACK_OVERFLOW，实测；10000 个操作数的空格链保持扁平从不溢出——证实深度
理论的差分探针）。

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
| `examples/` | `quickstart.rs` | 可运行的 DSL 主特性 demo（`cargo run --example quickstart`），14 段覆盖基础→复杂场景；另有 `simplify.rs`（约 15 行 DSL 生成 30 个 impl）与 `typeclass.rs`                    |
| `src/`      | 文件内 `#[cfg(test)]` 模块 | **234** 个单测（`cargo test --lib`）：按关注点就近放置，并带模块限定以便查找（`codegen::repeat_tests` 32、`codegen::range_worker` 19、`parse` 17、`codegen::sync` 15、`preprocess::varseg` 12、`preprocess::where_process` 12、`preprocess::angle` 9、`codegen::where_at_tests` 6、`testing::fuzz` 5、`codegen::top_level` 5、`preprocess::consts` 6、`ast::param_kind` 5……、`entry::impl_entry` 3） |
| `src/`      | `testing/`      | crate 级测试基建（全部 `cfg(test)`）：`fuzz.rs`（proptest——4 条属性 + 1 个用例，各 256 cases，随机 token 走真实入口，承诺不 panic）、`golden.rs`（2 个测试覆盖 10 份 `tests/golden/*.golden` 快照，`BLESS=1` 重写——**文本**锁：它钉住渲染出的 token 流，不保证该 token 流是合法 Rust，合法性由 `tests/features/*` 的编译承担）、`perf.rs`（展开开销测量）、`mod.rs`（`GuardAlloc` 256 MiB 分配守卫） |
| `tests/`    | `dsl.rs`        | 薄入口（`mod features;`）挂载拆分测试模块                                                                                                                                   |
| `tests/`    | `no_panic/main.rs` | no-panic 守卫：用 `syn` 走遍 `src/**/*.rs`，断言 `#[cfg(test)]` 之外无 panic 构造——包括**宏 token 流内部**铸出的（`quote!(x.unwrap())`）与**限定形式** `Option::unwrap(o)`——且任何属性位置、嵌在 `#[cfg_attr(…)]` 里或**宏体内**的 deny 家族 `#[allow]` / `#[expect]` 都被报出（一刀切静默同样在内：`clippy::all` / `clippy::restriction` / `warnings`）；`#[cfg(test)]` 闸门只跳过裸谓词，因此 `#[cfg(not(test))]` 的代码照样被扫描；crate 级 deny 行本身也被断言（`lib.rs` clippy deny 之外的第二条腿）；**检测器本身有自测**（`no_panic/selftest.rs`：每个臂都喂了合成违规 + 邻近反例，因此 `syn` 升级或收窄的 `matches!` 会让该文件失败而不是静默报 0 违规），唯一记录在案的洞是**宏体内手写的索引**逃过两条腿（`quote!(v[0])`——clippy 看不见宏体，而这里的 `[…]` 组无法与数组类型区分） |
| `tests/`    | `doc_consistency.rs` + `doc_consistency/reader_entry.rs` | **14** 项文档守卫：源码/模块树、文件路径、双语章节与示例、诊断和计数保持一致；README 入门程序必须能作为带 `main` 的独立 Rust 文件解析。源码导航检查独立 Markdown 锚点及仓库路径/片段，验收另查生成的 rustdoc 链接。历史 changelog 与 architecture 版本前言豁免当前文件引用检查 |
| `tests/`    | `features/`     | **61** 个按功能域拆分的测试模块（每个 ≤350 行；由原单文件 `dsl.rs` / `regression.rs` / `impl_entry_impl.rs` / `shape_template_impl.rs` 拆分），共 **382** 个 `#[test]`：`dsl_*`（运算符、限定类型、bound 位置的关联类型绑定、全局路径、fn 具名参数、指令、blanket、`@` 常量、`@N` 引用、参数包、where、泛型、接收者、入口宏、开放扩展、分发）、`regression_*`（角落用例 + `batch_impl` vs `batch_trait!` 一致性 + 宏/路径前缀 + 数组）、`impl_entry_*`（含嵌套/边界/冲突）、`shape_template_*`（含嵌套/边界/冲突/形状形态/原型模式/交叉组合 + 变长段与重复块）、另有 `dup_params` 与 `block_model` |
| `tests/`    | `ui.rs`         | `trybuild` UI 测试：**129** 个 `compile_fail` fixture 锁定诊断措辞 + 3 个 `pass` fixture |
| `tests/` | `pack_model/` | 独立 Pack 提案模型：Python 标准库，17 组测试、有限结构检查、双语教程核对与生成 Rust 的消费验证。执行 `python tests/pack_model/run.py`，独立 CI job 使用同一命令；不等于正式解析器或物化器的验证。 |

运行：

```bash
cargo run --example quickstart       # 主特性 demo
cargo test --lib                     # 单元测试 + fuzz
cargo test --test dsl                  # 功能 + 回归 + Ext 测试（tests/dsl.rs 挂载 tests/features/）
cargo test --test ui                  # 诊断 UI 测试
# 重新生成 UI 快照：
TRYBUILD=overwrite cargo test --test ui
```

## 发布流程

唯一权威是 `docs/development-guide.md` §3（四份 changelog 与 `## Unreleased`
占位规则、README / tutorial / reference / architecture 及其中英对应方的版本头同步、
`cargo package --list` 打包卫生，以及**CI 全绿后才 `cargo publish`**）。步骤顺序与
CI job 清单在那里，不在这里。
