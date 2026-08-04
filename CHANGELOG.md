# Changelog（用户）

> 用户可见的功能与行为变化；内部实现细节见 `docs/dev-changelog.md`。

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
- CHANGELOG 拆分为本文件（用户视角）与 `docs/dev-changelog.md`（开发者视角）
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
