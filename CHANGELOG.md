# Changelog

## 0.3.0 (2026-07-24)

### 用更合理的框架重写了 batch-impl

v0.3.0 是从零开始的完全重写。公开 API 和 DSL 语法与 v0.2.x 保持一致，
内部实现与旧版本无任何代码上的联系。

### 架构

```
lib.rs          宏入口（#[batch_impl] / batch_trait!）
  ├── parse.rs    DSL 解析器：Cursor 游标 + 优先级攀爬
  ├── types.rs    AST 节点（Ty 枚举 + 20 个变体）+ Op 优先级定义
  ├── apply.rs    运算符语义：apply() 折叠规则 + 元组展开
  └── codegen.rs  代码生成：Ty 递归拆解 → impl 块组装
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
