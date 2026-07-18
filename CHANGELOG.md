# Changelog

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
- macro-test：65 个测试用例
- ds-test：15 个边界测试
- 覆盖：基础类型、泛型、元组、`^` 运算符、`-` 运算符、unsafe、特殊类型
