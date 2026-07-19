# Changelog

## 0.2.0 (2026-07-19)

### 新功能

#### 关联类型简洁写法
- `TraitName<AssocType=value>` 语法：在 trait 泛型参数中指定关联类型绑定
- `<T> Iter<Item=T> Vec<T>` → 生成 `impl<T> Iter for Vec<T> { type Item = T; ... }`
- 支持多关联类型绑定：`Pair<First=T, Second=U>`
- 支持复杂类型绑定：`TupleAssoc<Output=(T, T)>`

#### 独立/共享 body 合并
- `[A{bodyA}, B{bodyB}]{shared}` 语法：列表项可有独立 body，与共享 body 合并
- 共享 body 提供公共实现，独立 body 提供类型特定实现
- 合并策略：拼接（shared + independent）
- 支持多层嵌套：`[[A{...}, B{...}]{shared1}, C{...}]{shared2}`

#### 实现细节
- `ImplSpec` 新增 `assoc_bindings` 字段
- `parse_segment` 解析 `TraitName<Item=T>` 时分离关联类型绑定（通过 `=` 检测）
- `parse_target` 支持独立 body 和共享 body 合并
- `generate_impl` 输出关联类型绑定到 impl 块

#### 测试
- 新增 14 个测试用例（76-81）
- 76: 关联类型 + unsafe
- 77: 关联类型 + 多类型实现
- 78: 关联类型 + 泛型约束
- 79: 关联类型 + 共享 body
- 80: 关联类型 + `^` 运算符
- 81: 关联类型 + `-` 运算符

#### Bug 修复
- 修复 `expand_caret` 和 `expand_dash` 不传递 `assoc_bindings` 的问题
- 现在所有功能可以任意组合：关联类型 + `^` + `-` + unsafe + 共享/独立 body

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
