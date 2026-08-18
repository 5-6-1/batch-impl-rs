
## ~~取消 batch_impl与batch_impl_only的自定义常量支持~~（0.8.0 已完成）

此为0.7.2误加，文档写明，不应该支持自定义常量，而是利用`^-*`的性质使其简化不重复写。
已于 0.8.0 回退：`ConstCtx::Attribute` 移除 `user_table`，属性宏内 `@name=value;` 定义段报
"custom constants are not supported"；`batch_trait!` 的支持保留。

## ~~Ext 1 语义任务清单（冻结）~~（0.8.0 已实现）

已落地：lib.rs 顶层分流（trait 分支不动）、`entry/impl_entry.rs`（shape 形态 `:` 分隔 + 直接形态、
`;` 分隔多 spec、零绑定 for-Type 校验、矩阵叶子展开、装配重写 for-Type/where/body、原始 impl
withhold、unsafe 保留）、`@`/`#` 域约束（仅 `@trait`；自定义常量 / `@N` / `@g_i` / `#` 指令定向报错，
`#[...]` 属性放行）、`where_process` 新增 `;` 停止与 `allow_end` 参数、共享 `codegen::shape` 内核。
语义修订（相对早期草案）：多 spec 用 `;` 分隔（`,` 归段内）；`@trait` 允许在 where 谓词；
body 内 `@trait` 保持原样（不报错——body 不在 attr 预处理范围，由 rustc 处理）。

### A. 入口位与分流

1. `#[batch_impl]` 在 `lib.rs::batch_impl` 里改 `parse_macro_input!(item as ItemTrait)` 为先嗅探 `item` 是 `Item::Trait` 还是 `Item::Impl`，二分流。
2. trait 分支保持现状（入口函数与现有签名不变）。
3. ItemImpl 分支为**新入口**，不替换、不改造 trait 入口。所有 ItemImpl 逻辑集中在新增的 `entry::impl_entry` 模块（命名待定，但**不写进 `parse_batch_trait_entry` / `generate_impl` / `run_pipeline` 三处共享内核**）。

### B. 属性语法（attr 内）

完整形式（两种 spec 形态；多 spec 用 `;` 分隔，单 spec 为常见形态）：
```
shape 形态：<shape-template> : <new-generic-decl>? <matrix-source>? (where <pred-list> | where{...})?
直接形态：<new-generic-decl>? <for-type> (where <pred-list> | where{...})?    // 无矩阵源，N=1
```

示例
```
#[batch_impl(A<B>:[Box,Rc]^[usize,isize]; <T>Box<T> where T:@trait)]
impl Tr for A<B>{
    const MAX:Self=A::new(B::MAX);
}
```

- 单条 statement 对应一个 ItemImpl；多 spec 用 `;` 分隔（`A:usize; A:isize`；用户定稿：不引入 `,` 分隔的多段——`,` 归段内 DSL 矩阵/参数），每条 spec 是独立生成任务，共享该 ItemImpl 的 trait path / for-Type / body。
- 字段含义无歧义锁定：
    - **shape-template**（shape 形态，`:` 前）：`syn::Type` 形态，作为绑定槽骨架。绑定规则与 Ext 2 `impl{...}` 模板同一内核（`codegen::shape::match_shape`）：模板与目标（矩阵叶子 / for-Type）**逐位比较**——对应位置**相同**（ident 文本一致）→ 该 ident 是字面，不进映射、不做替换；**不同** → 该 ident 是绑定槽，映射为目标对应子树。composite 只比较结构形状（generic arity、嵌套、分隔符、路径段数；不路径归一化），非 ident token 逐字相等。**必须与 ItemImpl 的 `for-Type` 整体同形**——只允许整体匹配，不允许只写 for-Type 的局部子树（零绑定校验，见 §I20）。
    - **new-generic-decl**：可选，形式 `< name : bound , ... >`。bound 位置允许写 `@trait`，展开期替换为 ItemImpl trait path 真值。无则整个 `<>` 省略。
    - **matrix-source**（shape 形态，`:` 后）：可选，现有矩阵 DSL（`^`/`-`/`[]`/`()`/`*`/`<>`）展开 N 个叶子。无则 N=1（空矩阵源 → 仅 1 个叶子即 shape 本身）。
    - **直接形态**（无 `:`）：`<new-generic-decl>?` 后直接是 for-type（即生成目标的 Self 类型），无绑定槽匹配，N=1；new-generic-decl 声明 for-type 引用的泛型参数。
    - **where 子句**：可选，**两种同源形式都允许**：
        - 裸 `where <pred-list>`（谓词序列，逗号分隔；区域到深度 0 `;` / 流末尾 / where ident / impl{} 停止）
        - `where{ <pred-list> }` 块容器形式
        - 两者生成结果同源：都产出最终的 impl 头 where 谓词。**`@trait` 在 where 谓词内合法**（展开为 trait path 真值，与 new-generic-decl bound 位置同规则）。
- **`;` 的作用**：spec 分隔符（多 spec：`A:u8; A:u16`）；同时是裸 where 谓词区域的停止边界（`;` 留在流中不消费）。trait 入口与 `batch_trait!` 共享该停止（`batch_trait!` 的 `;` 段边界顺带被修复）。
- **shape 新泛型与 where 同名空间**：shape-template 的绑定槽名、new-generic-decl 的参数名、matrix-source 的引用名、where 谓词里的引用名，共享一个名字空间。同名即同一实体（不引入"优先级"规则，只有"同一性"）。

### C. `@` 域约束（ItemImpl 入口）

4. 允许：
    - `@trait`（绑定到 ItemImpl trait path，在 new-generic-decl bound 位置与 where 谓词内消费）
5. 禁止（出现即报错）：
    - `@name=value;` 自定义常量（理由：无跨 trait 复用需求，应走 apply 系统）
    - `@all*` 选择器（`@all_fresh` / `@all_raw` / `@N..M` 等）——ItemImpl 入口没有 trait-aware fresh 重命名机制，无对应语义。
    - `@N` / `@g_i` 位置引用——同上，无 fresh 名引用对象。
    - `@trait<...>` 形式（这是 Ext 2 才涉及的）。
6. where 谓词内出现禁止的 `@` 构造 → 报错。

### D. `#` 域约束（ItemImpl 入口）

7. 禁止所有 `#` 指令（`#fill`/`#delegate`/`#blanket`/`#name{}`）——ItemImpl 入口无 trait 定义可指代，没有可负杂指令的目标。

### E. 生成顺序与产出

8. **先展开 impl 头，再展开体，用同一份替换映射**——头和体共享一份 `M`（shape-matching 结果），避免两次替换互相踩。
9. 流程（每叶子）：
    - shape-match（shape-template vs 叶子）→ 映射 `M`
    - 重写 impl 头的 for-Type：`for-Type` 按 `M` 替换绑定槽子树（非绑定槽逐字保留）
    - 重写 new-generic-decl（保留新泛型，`@trait` 替换为 trait path 真值）
    - 重写 where 谓词（**仅 impl 头 where**，谓词里的标识按需替换；where 子句**不进 body**）
    - 重写 ItemImpl body：body 里出现匹配映射 key 的标识按 `M` 替换
    - 装配：`impl<new generics> trait_path for rewritten_Type where ... { rewritten_body }`
    - emit 1 个 ItemImpl
10. N 个叶子 → N 个 ItemImpl。**原始 ItemImpl 被 withhold**（其 for-Type 含占位符不能编译）。

### F. where 与 body 严格分离

11. `where` 子句只作用于生成后的 impl 头。它**不**进入 body，不在 body 里做替换，不存在于 body 上下文。
12. body 的重写只走 shape 映射 `M`；body 里出现 `@trait` 直接报错（不是 body 通行占位符）。
13. body 不经过 DSL 预处理（避免 `*` splat 与 `*const T` 歧义）——用 `syn::parse` 常规解析后 `visit_mut` 替换。
14. **`where` 收集终止条件增强**：遇到 `;`、token 流末尾、**或 `impl{...}` 块**时停止收集 where 谓词。即 `impl{}` 与 `where{}` 同级作为 where 收集的边界。

### G. 预处理链路

15. ItemImpl 入口的 attr 走预处理子集：
- `angle_collect`（必须，配对 `<>`）
- `@trait` 替换（仅 new-generic bound 位，不全局 `@` 展开，因为 `@` 常量已禁）
- **不**走 `expand_tokens`（`#` 指令已禁）
- **不**走 `expand_empty_trait_generics`（无 ItemTrait）
- where 收集（增强版 `where_process`：识别 `;` 与流末尾边界）
16. ItemImpl 的 body 不进任何 DSL 预处理。
17. **`where_process` 的修改**：增强其 stop 条件，让遇到 `;` 或 token 流末尾时停止 where 谓词块收集。这一修改**同时服务于 trait 入口现有 `where{...}` 行为与 ItemImpl 新 naked-`where` 形式**（同源）。修改仅在收集器边界识别处加规则，**不改写主流程结构**。

### H. shape-matching 共享内核

18. 新增 `codegen::shape::match_shape(template: &syn::Type, leaf: &syn::Type) -> Result<Mapping, ShapeError>`：
    - **逐位递归**比较模板与叶子的对应节点（generic args 按位置、路径按段）：
      - 模板节点是 **ident**：与叶子对应节点文本**相同** → 字面，不进映射；**不同** → 绑定槽，映射为该 ident 名 → 叶子对应子树
      - 模板节点是 **composite**：叶子须同构（generic arity / 嵌套 / 分隔符 / 路径段数一致），否则 `ShapeMismatch`（不路径归一化）；内部 ident（base 与泛型参数）按上一条规则继续递归
      - 模板节点是**其他 token**（字面量等）：与叶子逐字相等，否则 `ShapeMismatch`
    - 同名槽已在映射里 → 新绑定的叶子子树须与旧子树同形，否则 `InconsistentBinding`
    - 0-arity（模板整体是裸 ident）→ 绑定整个叶子（`T := leaf`）
    - 语义依据（用户澄清，2025 定稿）："与将写的 impl 块的 Self 位置处匹配不同就替换，相同就不做处理"——同名 ident 是字面意图，异名 ident 是绑定槽。**H 节早期"composite token 逐字比较"表述作废。**
19. 该函数 Ext 1 与 Ext 2 共用，不耦合任何入口私有状态。

### I. shape-template 与 for-Type 校验

20. attr 解析期：shape-template 与 ItemImpl for-Type 跑一次 `match_shape`（template=attr shape, leaf=for-Type），失败报错（笔误兜底）。这一步的映射结果**不**用于 body 重写（body 重写用每叶子的映射，不是 for-Type 校验时的映射）——它只是形状合法性校验 + 笔误兜底。

### J. 整体测试策略

21. （落地实现时的测试，但语义列在这里：）
   - 单层矩阵、多维矩阵、`@trait` 绑定综合样例、同名与不同名两对照组、shape 不整同形报错、`@name` 禁用报错、body 内出现 `@trait` 报错、`#`/`impl{}` 禁用报错、两种 where 形式都生效且仅入头不进体、`;` 切 where 多谓词段。

### K. 失败模式与回滚

22. 风险：误分流伤及 trait 入口 → 顶层分流，trait 函数体不动；回归测试 1 条覆盖 `#[batch_impl] on trait` 仍正常。
23. 风险：`where_process` 改造破坏现有 trait 入口行为 → 改造只在 stop 条件处加新规则，不改主流程；现有 `where{...}` 行为加回归测试。
24. 回滚：所有改动集中新增 + `entry/mod.rs` 顶层分流 + `lib.rs::batch_impl` 改 `parse_macro_input!`；单 `git revert` 即可整体回退。

### L. 不做的事（阻 scope creep）

25. 不重构 trait 入口 pipeline。
26. 不改 `TyKind` 变体集合。
27. 不引入新依赖。
28. 不引入多 statement / 多 shape 语法——ItemImpl 入口一条 attr = 一个 shape = 一个生成 task（多矩阵叶子是其内置的 N 倍产出，不是多条 attr）。


## ~~Ext 2 语义任务清单（冻结）~~（0.8.0 已实现）

已落地：`codegen::shape` 共享内核（match_shape 逐位匹配，H 节语义）、`TyKind::WithImpl` 件、
`impl{...}` 任意顺序 attachment（split_trailing_body + 深度计数）、预处理判别（`expand_consts`
进入模板、`where_process` 视 `impl{...}` 为边界；判别中心化于 `util::is_impl_template`）、
codegen 多模板合并映射 + 目标/where/body 重写、4 个 ui fixture + tests/ext2_impl.rs（9 测试）。

### A. 入口位与定位

1. Ext 2 是**对现有 trait 入口（`#[batch_impl]` / `#[batch_impl_only]` / `batch_trait!`）的扩展**——不是新入口位。三个 trait 入口等价支持 Ext 2，无批差异。
2. Ext 2 与 Ext 1 共用 shape-matching 内核 `codegen::shape::match_shape`（Ext 1 §H 已写）。

### B. 语法：`impl{...}` Self-part 容器

示例
```
#[batch_impl([Box,Rc]^[isize,usize,<T:@trait>T]impl{A<B>}#MAX{A::new(B::MAX)})]
trait Tr{const MAX:Self;}
```

3. 现有 trait 入口 spec 形态里加一个**可选的 `impl{...}` 块**，作为 `Self`-part（即 for-Type）的形状绑定容器，与现有 `where{...}` 和 `{body}` 并列。
4. 三种 attachment（`impl{...}` / `where{...}` / `{body}`）**任意顺序**（G 节 §18 定稿；早期 B 节草案的固定顺序"impl{} 永远在 where{} 前"已作废），解析器按出现顺序识别，不引入组合爆炸（不可任意调换顺序的约束取消）。
5. 三种 attachment 语义分工：
    - `{body}` = 方法体（已存在）
    - `where{...}` = where 谓词容器（已存在）
    - `impl{...}` = Self-part 形状绑定（新增）

### C. `impl{...}` 内部内容约束

6. `impl{...}` 块内容是**一段标准 Rust `syn::Type`**（用 `syn::parse` 解析）。其内部**不**走 DSL 预处理——syn 直接拒绝 DSL 专用算子（`^`/`-`/`*` splat/`#`），天然无歧义。
7. `impl{}` 内允许 `@` 常量展开（`@trait` 等）。**例外**：`@` 常量在 `impl{}` 块内走预处理的 `expand_consts` 阶段展开（§H20），展开后留下的是标准类型 token，再喂给 `syn::parse`。
8. **`impl{}` 内只能是标准类型形态**——即 `A<B>`、`Rc<T>`、`Box<Vec<T>>`、path、ref、array、fn 类型等。**禁止**在 `impl{}` 内出现：
    - 前置 `<>` 约束（如 `<T: @trait>` 形式的 new-generic-decl）
    - 后置 `where` 约束
    - 矩阵 DSL 算子（`^`/`-`/`*`/`[]`/`()`）
    - `#` 指令
      syn 在 `parse::<Type>` 阶段就会拒绝大多数；前置 `<>` 与后置 `where` 这种"看着像类型但实际是 impl 头部片段"的形态，需在展开后/解析前显式校验并报错（不存在于合法 `syn::Type` 的 token 模式由校验兜底）。

### D. `impl{}` 内的 `@` 常量展开

9. `impl{}` 块内 `@` 常量展开有两种来源：
    - trait 入口自带的全局 `@name=value;` 自定义常量（用户可在 attr 顶部定义，`impl{}` 内引用）
    - `@trait`（绑定到当前 trait 入口的 trait path 真值）
10. 展开发生在**预处理阶段**（`expand_consts`），不在 `syn::parse` 之后。流程：`impl{}` 块进 `expand_consts` → `@trait`/`@name` 替换为对应 token → 留下纯标准类型 token → `syn::parse::<Type>` → 进入 shape-match。

### E. impl{} 模板与矩阵 leaf 的匹配

11. `impl{...}` 内是模板：bare ident = 绑定槽，composite 节点逐字结构比较（与 Ext 1 共用 `match_shape`）。
12. 一条 spec 内**先展开矩阵**得 N 个 leaf，**再对每个 leaf** 跑 `impl{}` 的 shape-match。`impl{}` 不跨 leaf 共享映射——单 leaf 单映射。
13. `impl{}` 模板与生成 leaf 的 for-Type 跑 `match_shape`：
    - 成功 → `Self`-part 替换映射 `M_impl`
    - 失败 → 报错（"for-Type cannot be destructured by `impl{...}` template"）
14. leaf 中 `impl{}` 不匹配 for-Type 的报错时机：与现有 spec 错误聚合一致，**所有 leaf 都跑，一次性报齐**（不首错即停）。

### F. 多 `impl{}` 合并规则

15. 允许一个 leaf 上挂多个 `impl{...}`（attachment chain）：
    ```
    T impl{...} impl{...} where{...} {body} // 任意顺序与数量
    ```
    多个 `impl{}` 的形状绑定**合并为单一替换映射 `M_impl`**，产出仍然只**一个** impl block。
16. 合并冲突规则：同名绑定槽在不同 `impl{}` 里映射到**不同叶子子树** → 报 `InconsistentBinding`（无 override 语义，不允许后挂覆盖前挂）。
17. 同名绑定槽在不同 `impl{}` 里映射到**同形子树** → 合并保留（冗余但合法）。

### G. attachment 任意顺序

18. `impl{...}`、`where{...}`、`{body}` 三种 attachment **任意顺序**，解析器按出现顺序识别，**不再强制固定顺序**。三者parse同期处理。
19. 多 `impl{}` 之间顺序对映射结果无差别（同名同形视为冗余保留，同名异形视为冲突报错——与先后无涉）。

### H. attachment 深度计数

20. attachment 链深度计必须**把 `impl{}` 也计入**。现有 trait 入口 attachment 深度上限 128（针对 `where{...}`/`{body}` 链）；Ext 2 加上 `impl{}` 后三者在同一深度链上，**总深仍上限 128**（不分项计数）。
21. 深度超限报错（与现有 "trailing attachment chain exceeds 128" 同质，仅扩展报错信息覆盖 `impl{}`）。

### I. 名字空间：绑定槽名与泛型名同名

22. `impl{}` 内绑定槽名与本 spec 内的泛型引用名（trait 入口的 spec-level `Self<T>` 之类里的 `T`）**共享一个名字空间**。同名即同一实体——替换结果无差别，视作同一内容。
23. 不引入"同名优先级"规则，只有"同一性"判定。

### J. `impl{}` 替换映射与 body 重写的关系

24. `impl{}` 的替换映射 `M_impl` **会落入 body 重写**——body 走 `visit_mut` 替换时，命中 `M_impl` key 的标识按映射替换。**与 Ext 1 同行为**：默认替换，后处理无从也不去区分标识来源（不区分是来自指令系统还是来自 `impl{}` 模板）。
25. `#name`-copied 的 trait 方法签名里的方法名：**会被替换**（与 §J24 同——body 替换一律执行，包括方法名）。**确认：与 Ext 1 行为完全相同，不保留 trait 方法名。**

### K. `impl{}` 替换映射与 where 谓词、impl 头的关系

26. `M_impl` 用于**约束并重写生成 impl 的 for-Type 形状**——这是 `impl{}` 的首要作用。
27. `M_impl` 也用于 where 谓词的标识替换（where 谓词里的标识命中 `M_impl` key 则替换）。与 body 同映射，避免双套映射。
28. `M_impl` 不参与 `impl<>` 泛型参数声明本身（即不替换 new-generic 参数名本身）——`impl{}` 只是 Self-part 形状绑定，不改泛型声明。

### L. 预处理四道与 `impl{}` 关系

29. 预处理四道对 `impl{}` 块的处理策略**分歧**：
    - `expand_consts`：**进入** `impl{}` 块 —— 块内 `@trait`/`@name=value` 需要展开
    - `angle_collect`：**不进入** `impl{}` 块 —— 块内 `<...>` 保持扁平，后续直接喂 `syn::parse::<Type>`，由 syn 处理泛型参数配对
    - `expand_tokens`：`impl{}` 块内 **passthrough**（不展开 `#` 指令，§C8 禁止）
    - `where_process`：`impl{}` 块内 **passthrough**（不收集 where，§C8 禁止）
30. **中心化"是否进入该 delimiter 组"的判别函数**：提取共用判别，让四道都走它，防漂移。判别返回 `EnterPolicy { expand_consts: bool, angle_collect: bool, expand_tokens: bool, where_process: bool }`。
31. `impl{}` 块进 `expand_consts` 后，其内部 `@trait` 替换为当前 trait 入口 trait path 真值，`@name=value` 自定义常量按全局 `@` 表展开。展开后留下纯标准类型 token，再喂 `syn::parse::<Type>`。
32. `#` 指令**不进** `impl{}`（`impl{}` 内禁止 `#`，§C8）。
33. `where_process` 在 `impl{}` 块内 passthrough（`impl{}` 内不允许 `where`，§C8）。

### M. `impl{}` AST 件

34. `impl{}` 形态在 AST 上需要一个表示（暂名 `TyKind::WithImpl` 或类似），承载 `(Option<Box<Ty>>, syn::Type template)`——inner `Ty` 是被绑定的 leaf 端，`syn::Type` 是模板。**【实现期决策】**：可能借既有 wrapper 模板（如 `WithCode`/`WithWhere`）的 (Option, payload) 同构形态，不重新设计 AST 形态。
35. `map_children` 安全网必须覆盖新件——所有递归点（apply / render / visit / postprocess）必须显式处理 `impl{}`，否则 `map_children` exhaustive match 会编译失败强制补齐。
36. `impl{}` 件在 `render` 阶段：把模板按 `M_impl` 替换后渲染为最终 for-Type token 流。其余 where / body 部分按既有 render 流程附带。

### N. 共享内核延伸

37. `codegen::shape::match_shape`（Ext 1 §H 锁定的）在 Ext 2 被调用：`match_shape(impl_template: &syn::Type, leaf_for_type: &syn::Type) -> Result<Mapping, ShapeError>`。
38. `Mapping` 数据结构 Ext 1/Ext 2 共用（槽名 → 子树），不二份实现。

### O. Ext 2 测试策略（语义列在此，实现期落地）

39. 测试用例：
    - `impl{T}` + i32 → `T := i32`
    - `impl{Rc<T>}` + `Rc<i32>` → `T := i32`
    - `impl{Rc<T>}` + `Box<i32>` → `Rc:=Box, T:=i32`（尽管这可能是错误的）
    - `impl{@trait<T>}` → `@trait` 在 `impl{}` 内展开为 trait path 真值，再 match
    - 多 `impl{}` 合并：同形冗余合法、异形冲突报错
    - 乱序 `T where{...} impl{...}` → 合法（attachment 任意顺序）
    - 深度超 128 → 报错
    - `impl{}` 内出现 `<T: Bound>Box<T>`（前置 `<>`）→ 报错
    - `impl{}` 内出现 `where T: Clone`（后置 `where`）→ 报错
    - `impl{}` 内出现 DSL 算子 `^` `*` 等 → syn 拒绝（自然报错）
    - `#name`-copied 方法名在 `impl{}` 映射 key 里时**被替换**（验证 §J25）
    - `impl{}` 映射落入 body 替换（验证 §J24）

### P. 失败模式与回滚

40. 风险：预处理四道漏加 `impl{}` 分支 → 复现 0.5.7 `#[...]` 漏处理 bug。对策：中心化判别函数 + 单测覆盖 `@` 常量在 `impl{}` 块内的展开。
41. 风险：attachment 顺序固定破坏现有 `{body}`/`where{}` 链解析。对策：现有解析逻辑**只加 `impl{}` 识别**，不重写判定顺序。
42. 风险：`map_children` 未覆盖 `impl{}` 件 → 编译失败（这是安全网本身的作用，强制补齐，非运行时风险）。
43. 回滚：所有改动集中在 `parse/trailing` + 预处理四道判别中心 + 新增 `impl_part` AST 件 + `codegen::shape`；单 `git revert` 即可整体回退。

### Q. 不做的事（阻 scope creep）

44. 不改 trait 入口主流程结构，仅加 `impl{}` 件与判别。
45. 不引入新依赖。
46. 不改 Ext 1（Ext 1 与 Ext 2 共享 `codegen::shape`，但不互改对方入口）。
47. 不引入 `impl{}` body 重写规则变更——沿用 Ext 1 的 body 替换同行为。


