# batch-impl 内部架构

面向贡献者：模块组织、解析流程、错误机制、测试矩阵。

## 模块组织

```text
lib.rs              宏入口（#[batch_impl] / #[batch_impl_only] / batch_trait! / 测试宏）
  ├── expand.rs             入口实现：expand_attr_macro / expand_batch_trait + 公共管线 run_pipeline
  ├── batch_trait_entry.rs  共享驱动：BFS 展开并列列表 → 逐叶子 generate_impl
  ├── trait_bounds.rs       TraitBounds / TraitParam + syn AST 引用收集（where 谓词透传槽位）
  ├── empty_generics.rs     `A<>` 照抄展开（形参渲染用合并后的 bound）
  ├── path_prefix.rs        外部 trait 路径前缀：#Path::to::Trait: 状态机解析
  ├── diagnostic.rs         统一 compile_error_str(msg) 用于编译期诊断
  ├── scan.rs               扫描与游标：Cursor<'a> + scan_stop（尖括号已配对，仅剩 -> 守卫）
  ├── parse/                解析层
  │   ├── mod.rs            DSL 解析器：优先级攀爬（Op::Semi/Comma/Dash/Caret/Prim）
  │   ├── parse_atom.rs     原子层解析：属性 / fn / 前缀 / 范围 / 分组 / 列表
  │   └── generic.rs        泛型解析：parse_generic / parse_angle_bracket_contents（尖括号组即 delimiter![<>]）
  ├── preprocess/           预处理层
  │   ├── mod.rs            delimiter! 分隔符拼写宏 + 指令预处理：#name 指令展开（内置 + 开放扩展）
  │   ├── preprocess_helpers.rs  预处理辅助：build_from_item / get_trait_item / parse_names_from_tokens（列表减法 `-`）
  │   ├── where_process.rs  裸 where 改写：`where 谓词 {body}` → 旧式 `where{谓词}`
  │   └── angle.rs          尖括号组：入口 None 组扁平化 + `<...>` 配对为组（输出侧还原），parse 层不再管 <> 深度
  ├── ast/                  AST 层
  │   ├── mod.rs            Ty 枚举（18 个变体，含 Error）+ Op 优先级定义
  │   └── types_render.rs   AST 渲染：ToTokens impl for Ty + params_to_tokens 系列
  ├── apply/                运算层
  │   ├── mod.rs            Apply trait + 核心 apply() 两阶段分发（右操作数"结构"优先）
  │   └── apply_tuple.rs    元组与容器运算符 + 元组展开（^N / 笛卡尔积 / 范围 / fresh 泛型）
  └── codegen/
      └── mod.rs            代码生成：extract_impl_parts → hoist_type_params → generate_impl（含 where 谓词附加与引用检查）
```

## 解析流程

**token 流 → 指令预处理（每条指令展开为恰好一个 `{...}` 组）→ where 裸写改写
→ Cursor 扫描取切片 → parse_item 优先级攀爬（`^`/`-` 经 `Apply` 组合：
右操作数结构优先分发）→ Ty AST → 工作清单摊平并列列表 → 逐叶子 generate_impl**

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
  HRTB binder（`for<'a>`）天然排除；const 泛型实参 / 数组长度经 `visit_expr`
  收集。`impl_names` 中 `const N` 归一如 `N` 以匹配引用检查。

## 错误机制

所有 DSL 语法错误均通过 `compile_error!()` 输出友好的编译错误，**永不 panic**。
两层分工，不合并：

- **DSL 解析层**（parse/apply/codegen）：`Ty::Error` 变体在 AST 链中透传
  （链式组合中途失败需要信号值），最终经 ToTokens 输出 `compile_error!`；
- **入口层**（preprocess/expand）：`Result<_, TokenStream>` 经 `?` 传播，
  由 `diagnostic.rs::compile_error_str` 统一构造。

## 测试矩阵

四层：

| 目录            | 文件             | 用途                                                                                                                                     |
|-----------------|------------------|------------------------------------------------------------------------------------------------------------------------------------------|
| `examples/`     | `quickstart.rs`  | 可运行的 DSL 主特性 demo（`cargo run --example quickstart`），14 段覆盖基础→复杂场景                                                      |
| `src/`          | `fuzz.rs`        | proptest 属性测试：随机 token 序列喂 `where_process` / `parse_item`，验证"不因用户输入 panic"（`cargo test --lib`）                       |
| `tests/`        | `dsl.rs`         | 34 个 `#[test]`，覆盖核心特性的语义回归（含 where 子句继承、外部路径前缀、宏调用边界、`unsafe fn` 类型、列表减法 `-`、`A<>` 与同名继承） |
| `tests/`        | `regression.rs`  | 23 个 `#[test]`，覆盖 dsl.rs 未触碰的 corner case：嵌套 `>>`、路径类型、const 泛型、生命周期、dyn + Send、路径前缀、数组/切片 builder、`batch_impl` vs `batch_trait!` 一致性 |
| `tests/`        | `ui.rs`          | `trybuild` UI 测试：23 个 `compile_fail` fixture 锁定诊断措辞 + 1 个 `pass` fixture |

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

1. CHANGELOG 按"用户 / 开发者"两节记录
2. `cargo package` 验证打包（docs/ 目录随 git 跟踪自动入包）
3. `cargo publish`
4. `git tag vX.Y.Z && git push origin vX.Y.Z`
5. `gh release create vX.Y.Z --notes-file <notes>`
